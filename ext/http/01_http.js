// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
const core = globalThis.Deno.core;
const internals = globalThis.__bootstrap.internals;
const primordials = globalThis.__bootstrap.primordials;
const { BadResourcePrototype, InterruptedPrototype, ops } = core;
import * as webidl from "ext:deno_webidl/00_webidl.js";
import { InnerBody } from "ext:deno_fetch/22_body.js";
import { Event, setEventTargetData } from "ext:deno_web/02_event.js";
import { BlobPrototype } from "ext:deno_web/09_file.js";
import {
  fromInnerResponse,
  newInnerResponse,
  ResponsePrototype,
  toInnerResponse,
} from "ext:deno_fetch/23_response.js";
import {
  fromInnerRequest,
  newInnerRequest,
  toInnerRequest,
} from "ext:deno_fetch/23_request.js";
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
import { listen, TcpConn, UnixConn } from "ext:deno_net/01_net.js";
import { listenTls, TlsConn } from "ext:deno_net/02_tls.js";
import {
  Deferred,
  getReadableStreamResourceBacking,
  readableStreamClose,
  readableStreamForRid,
  ReadableStreamPrototype,
} from "ext:deno_web/06_streams.js";
const {
  ArrayPrototypeIncludes,
  ArrayPrototypeMap,
  ArrayPrototypePush,
  Error,
  ObjectPrototypeIsPrototypeOf,
  SafeSetIterator,
  Set,
  SetPrototypeAdd,
  SetPrototypeDelete,
  SetPrototypeClear,
  StringPrototypeCharCodeAt,
  StringPrototypeIncludes,
  StringPrototypeToLowerCase,
  StringPrototypeSplit,
  SafeSet,
  PromisePrototypeCatch,
  Symbol,
  SymbolAsyncIterator,
  TypeError,
  Uint8Array,
  Uint8ArrayPrototype,
} = primordials;

const connErrorSymbol = Symbol("connError");
const streamRid = Symbol("streamRid");
const _deferred = Symbol("upgradeHttpDeferred");

class HttpConn {
  #rid = 0;
  #closed = false;
  #remoteAddr;
  #localAddr;

  // This set holds resource ids of resources
  // that were created during lifecycle of this request.
  // When the connection is closed these resources should be closed
  // as well.
  managedResources = new Set();

  constructor(rid, remoteAddr, localAddr) {
    this.#rid = rid;
    this.#remoteAddr = remoteAddr;
    this.#localAddr = localAddr;
  }

  /** @returns {number} */
  get rid() {
    return this.#rid;
  }

  /** @returns {Promise<RequestEvent | null>} */
  async nextRequest() {
    let nextRequest;
    try {
      nextRequest = await core.opAsync("op_http_accept", this.#rid);
    } catch (error) {
      this.close();
      // A connection error seen here would cause disrupted responses to throw
      // a generic `BadResource` error. Instead store this error and replace
      // those with it.
      this[connErrorSymbol] = error;
      if (
        ObjectPrototypeIsPrototypeOf(BadResourcePrototype, error) ||
        ObjectPrototypeIsPrototypeOf(InterruptedPrototype, error) ||
        StringPrototypeIncludes(error.message, "connection closed")
      ) {
        return null;
      }
      throw error;
    }
    if (nextRequest == null) {
      // Work-around for servers (deno_std/http in particular) that call
      // `nextRequest()` before upgrading a previous request which has a
      // `connection: upgrade` header.
      await null;

      this.close();
      return null;
    }

    const { 0: streamRid, 1: method, 2: url } = nextRequest;
    SetPrototypeAdd(this.managedResources, streamRid);

    /** @type {ReadableStream<Uint8Array> | undefined} */
    let body = null;
    // There might be a body, but we don't expose it for GET/HEAD requests.
    // It will be closed automatically once the request has been handled and
    // the response has been sent.
    if (method !== "GET" && method !== "HEAD") {
      body = readableStreamForRid(streamRid, false);
    }

    const innerRequest = newInnerRequest(
      method,
      url,
      () => ops.op_http_headers(streamRid),
      body !== null ? new InnerBody(body) : null,
      false,
    );
    innerRequest[streamRid] = streamRid;
    const abortController = new AbortController();
    const request = fromInnerRequest(
      innerRequest,
      abortController.signal,
      "immutable",
      false,
    );

    const respondWith = createRespondWith(
      this,
      streamRid,
      request,
      this.#remoteAddr,
      this.#localAddr,
      abortController,
    );

    return { request, respondWith };
  }

  /** @returns {void} */
  close() {
    if (!this.#closed) {
      this.#closed = true;
      core.close(this.#rid);
      for (const rid of new SafeSetIterator(this.managedResources)) {
        SetPrototypeDelete(this.managedResources, rid);
        core.close(rid);
      }
    }
  }

  [SymbolAsyncIterator]() {
    // deno-lint-ignore no-this-alias
    const httpConn = this;
    return {
      async next() {
        const reqEvt = await httpConn.nextRequest();
        // Change with caution, current form avoids a v8 deopt
        return { value: reqEvt ?? undefined, done: reqEvt === null };
      },
    };
  }
}

function createRespondWith(
  httpConn,
  streamRid,
  request,
  remoteAddr,
  localAddr,
  abortController,
) {
  return async function respondWith(resp) {
    try {
      resp = await resp;
      if (!(ObjectPrototypeIsPrototypeOf(ResponsePrototype, resp))) {
        throw new TypeError(
          "First argument to respondWith must be a Response or a promise resolving to a Response.",
        );
      }

      const innerResp = toInnerResponse(resp);

      // If response body length is known, it will be sent synchronously in a
      // single op, in other case a "response body" resource will be created and
      // we'll be streaming it.
      /** @type {ReadableStream<Uint8Array> | Uint8Array | null} */
      let respBody = null;
      if (innerResp.body !== null) {
        if (innerResp.body.unusable()) {
          throw new TypeError("Body is unusable.");
        }
        if (
          ObjectPrototypeIsPrototypeOf(
            ReadableStreamPrototype,
            innerResp.body.streamOrStatic,
          )
        ) {
          if (
            innerResp.body.length === null ||
            ObjectPrototypeIsPrototypeOf(
              BlobPrototype,
              innerResp.body.source,
            )
          ) {
            respBody = innerResp.body.stream;
          } else {
            const reader = innerResp.body.stream.getReader();
            const r1 = await reader.read();
            if (r1.done) {
              respBody = new Uint8Array(0);
            } else {
              respBody = r1.value;
              const r2 = await reader.read();
              if (!r2.done) throw new TypeError("Unreachable");
            }
          }
        } else {
          innerResp.body.streamOrStatic.consumed = true;
          respBody = innerResp.body.streamOrStatic.body;
        }
      } else {
        respBody = new Uint8Array(0);
      }
      const isStreamingResponseBody = !(
        typeof respBody === "string" ||
        ObjectPrototypeIsPrototypeOf(Uint8ArrayPrototype, respBody)
      );
      try {
        await core.opAsync(
          "op_http_write_headers",
          streamRid,
          innerResp.status ?? 200,
          innerResp.headerList,
          isStreamingResponseBody ? null : respBody,
        );
      } catch (error) {
        const connError = httpConn[connErrorSymbol];
        if (
          ObjectPrototypeIsPrototypeOf(BadResourcePrototype, error) &&
          connError != null
        ) {
          // deno-lint-ignore no-ex-assign
          error = new connError.constructor(connError.message);
        }
        if (
          respBody !== null &&
          ObjectPrototypeIsPrototypeOf(ReadableStreamPrototype, respBody)
        ) {
          await respBody.cancel(error);
        }
        throw error;
      }

      if (isStreamingResponseBody) {
        let success = false;
        if (
          respBody === null ||
          !ObjectPrototypeIsPrototypeOf(ReadableStreamPrototype, respBody)
        ) {
          throw new TypeError("Unreachable");
        }
        const resourceBacking = getReadableStreamResourceBacking(respBody);
        let reader;
        if (resourceBacking) {
          if (respBody.locked) {
            throw new TypeError("ReadableStream is locked.");
          }
          reader = respBody.getReader(); // Aquire JS lock.
          try {
            await core.opAsync(
              "op_http_write_resource",
              streamRid,
              resourceBacking.rid,
            );
            if (resourceBacking.autoClose) core.tryClose(resourceBacking.rid);
            readableStreamClose(respBody); // Release JS lock.
            success = true;
          } catch (error) {
            const connError = httpConn[connErrorSymbol];
            if (
              ObjectPrototypeIsPrototypeOf(BadResourcePrototype, error) &&
              connError != null
            ) {
              // deno-lint-ignore no-ex-assign
              error = new connError.constructor(connError.message);
            }
            await reader.cancel(error);
            throw error;
          }
        } else {
          reader = respBody.getReader();
          while (true) {
            const { value, done } = await reader.read();
            if (done) break;
            if (!ObjectPrototypeIsPrototypeOf(Uint8ArrayPrototype, value)) {
              await reader.cancel(new TypeError("Value not a Uint8Array"));
              break;
            }
            try {
              await core.opAsync2("op_http_write", streamRid, value);
            } catch (error) {
              const connError = httpConn[connErrorSymbol];
              if (
                ObjectPrototypeIsPrototypeOf(BadResourcePrototype, error) &&
                connError != null
              ) {
                // deno-lint-ignore no-ex-assign
                error = new connError.constructor(connError.message);
              }
              await reader.cancel(error);
              throw error;
            }
          }
          success = true;
        }

        if (success) {
          try {
            await core.opAsync("op_http_shutdown", streamRid);
          } catch (error) {
            await reader.cancel(error);
            throw error;
          }
        }
      }

      const deferred = request[_deferred];
      if (deferred) {
        const res = await core.opAsync("op_http_upgrade", streamRid);
        let conn;
        if (res.connType === "tcp") {
          conn = new TcpConn(res.connRid, remoteAddr, localAddr);
        } else if (res.connType === "tls") {
          conn = new TlsConn(res.connRid, remoteAddr, localAddr);
        } else if (res.connType === "unix") {
          conn = new UnixConn(res.connRid, remoteAddr, localAddr);
        } else {
          throw new Error("unreachable");
        }

        deferred.resolve([conn, res.readBuf]);
      }
      const ws = resp[_ws];
      if (ws) {
        const wsRid = await core.opAsync(
          "op_http_upgrade_websocket",
          streamRid,
        );
        ws[_rid] = wsRid;
        ws[_protocol] = resp.headers.get("sec-websocket-protocol");

        httpConn.close();

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
      }
    } catch (error) {
      abortController.abort(error);
      throw error;
    } finally {
      if (SetPrototypeDelete(httpConn.managedResources, streamRid)) {
        core.close(streamRid);
      }
    }
  };
}

const _ws = Symbol("[[associated_ws]]");
const websocketCvf = buildCaseInsensitiveCommaValueFinder("websocket");
const upgradeCvf = buildCaseInsensitiveCommaValueFinder("upgrade");

function upgradeWebSocket(request, options = {}) {
  const upgrade = request.headers.get("upgrade");
  const upgradeHasWebSocketOption = upgrade !== null &&
    websocketCvf(upgrade);
  if (!upgradeHasWebSocketOption) {
    throw new TypeError(
      "Invalid Header: 'upgrade' header must contain 'websocket'",
    );
  }

  const connection = request.headers.get("connection");
  const connectionHasUpgradeOption = connection !== null &&
    upgradeCvf(connection);
  if (!connectionHasUpgradeOption) {
    throw new TypeError(
      "Invalid Header: 'connection' header must contain 'Upgrade'",
    );
  }

  const websocketKey = request.headers.get("sec-websocket-key");
  if (websocketKey === null) {
    throw new TypeError(
      "Invalid Header: 'sec-websocket-key' header must be set",
    );
  }

  const accept = ops.op_http_websocket_accept_header(websocketKey);

  const r = newInnerResponse(101);
  r.headerList = [
    ["upgrade", "websocket"],
    ["connection", "Upgrade"],
    ["sec-websocket-accept", accept],
  ];

  const protocolsStr = request.headers.get("sec-websocket-protocol") || "";
  const protocols = StringPrototypeSplit(protocolsStr, ", ");
  if (protocols && options.protocol) {
    if (ArrayPrototypeIncludes(protocols, options.protocol)) {
      ArrayPrototypePush(r.headerList, [
        "sec-websocket-protocol",
        options.protocol,
      ]);
    } else {
      throw new TypeError(
        `Protocol '${options.protocol}' not in the request's protocol list (non negotiable)`,
      );
    }
  }

  const response = fromInnerResponse(r, "immutable");

  const socket = webidl.createBranded(WebSocket);
  setEventTargetData(socket);
  socket[_server] = true;
  response[_ws] = socket;
  socket[_idleTimeoutDuration] = options.idleTimeout ?? 120;
  socket[_idleTimeoutTimeout] = null;

  return { response, socket };
}

function upgradeHttp(req) {
  req[_deferred] = new Deferred();
  return req[_deferred].promise;
}

async function upgradeHttpRaw(req, tcpConn) {
  const inner = toInnerRequest(req);
  const res = await core.opAsync("op_http_upgrade_early", inner[streamRid]);
  return new TcpConn(res, tcpConn.remoteAddr, tcpConn.localAddr);
}

const spaceCharCode = StringPrototypeCharCodeAt(" ", 0);
const tabCharCode = StringPrototypeCharCodeAt("\t", 0);
const commaCharCode = StringPrototypeCharCodeAt(",", 0);

/** Builds a case function that can be used to find a case insensitive
 * value in some text that's separated by commas.
 *
 * This is done because it doesn't require any allocations.
 * @param checkText {string} - The text to find. (ex. "websocket")
 */
function buildCaseInsensitiveCommaValueFinder(checkText) {
  const charCodes = ArrayPrototypeMap(
    StringPrototypeSplit(
      StringPrototypeToLowerCase(checkText),
      "",
    ),
    (c) => [c.charCodeAt(0), c.toUpperCase().charCodeAt(0)],
  );
  /** @type {number} */
  let i;
  /** @type {number} */
  let char;

  /** @param value {string} */
  return function (value) {
    for (i = 0; i < value.length; i++) {
      char = value.charCodeAt(i);
      skipWhitespace(value);

      if (hasWord(value)) {
        skipWhitespace(value);
        if (i === value.length || char === commaCharCode) {
          return true;
        }
      } else {
        skipUntilComma(value);
      }
    }

    return false;
  };

  /** @param value {string} */
  function hasWord(value) {
    for (let j = 0; j < charCodes.length; ++j) {
      const { 0: cLower, 1: cUpper } = charCodes[j];
      if (cLower === char || cUpper === char) {
        char = StringPrototypeCharCodeAt(value, ++i);
      } else {
        return false;
      }
    }
    return true;
  }

  /** @param value {string} */
  function skipWhitespace(value) {
    while (char === spaceCharCode || char === tabCharCode) {
      char = StringPrototypeCharCodeAt(value, ++i);
    }
  }

  /** @param value {string} */
  function skipUntilComma(value) {
    while (char !== commaCharCode && i < value.length) {
      char = StringPrototypeCharCodeAt(value, ++i);
    }
  }
}

// Expose this function for unit tests
internals.buildCaseInsensitiveCommaValueFinder =
  buildCaseInsensitiveCommaValueFinder;

function hostnameForDisplay(hostname) {
  // If the hostname is "0.0.0.0", we display "localhost" in console
  // because browsers in Windows don't resolve "0.0.0.0".
  // See the discussion in https://github.com/denoland/deno_std/issues/1165
  return hostname === "0.0.0.0" ? "localhost" : hostname;
}

async function respond(handler, requestEvent, connInfo, onError) {
  let response;

  try {
    response = await handler(requestEvent.request, connInfo);

    if (response.bodyUsed && response.body !== null) {
      throw new TypeError("Response body already consumed.");
    }
  } catch (e) {
    // Invoke `onError` handler if the request handler throws.
    response = await onError(e);
  }

  try {
    // Send the response.
    await requestEvent.respondWith(response);
  } catch {
    // `respondWith()` can throw for various reasons, including downstream and
    // upstream connection errors, as well as errors thrown during streaming
    // of the response content.  In order to avoid false negatives, we ignore
    // the error here and let `serveHttp` close the connection on the
    // following iteration if it is in fact a downstream connection error.
  }
}

async function serveConnection(
  server,
  activeHttpConnections,
  handler,
  httpConn,
  connInfo,
  onError,
) {
  while (!server.closed) {
    let requestEvent = null;

    try {
      // Yield the new HTTP request on the connection.
      requestEvent = await httpConn.nextRequest();
    } catch {
      // Connection has been closed.
      break;
    }

    if (requestEvent === null) {
      break;
    }

    respond(handler, requestEvent, connInfo, onError);
  }

  SetPrototypeDelete(activeHttpConnections, httpConn);
  try {
    httpConn.close();
  } catch {
    // Connection has already been closed.
  }
}

async function serve(arg1, arg2) {
  let options = undefined;
  let handler = undefined;
  if (typeof arg1 === "function") {
    handler = arg1;
    options = arg2;
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

  const signal = options.signal;
  const onError = options.onError ?? function (error) {
    console.error(error);
    return new Response("Internal Server Error", { status: 500 });
  };
  const onListen = options.onListen ?? function ({ port }) {
    console.log(
      `Listening on http://${hostnameForDisplay(listenOpts.hostname)}:${port}/`,
    );
  };
  const listenOpts = {
    hostname: options.hostname ?? "127.0.0.1",
    port: options.port ?? 9000,
    reusePort: options.reusePort ?? false,
  };

  if (options.cert || options.key) {
    if (!options.cert || !options.key) {
      throw new TypeError(
        "Both cert and key must be provided to enable HTTPS.",
      );
    }
    listenOpts.cert = options.cert;
    listenOpts.key = options.key;
  }

  let listener;
  if (listenOpts.cert && listenOpts.key) {
    listener = listenTls({
      hostname: listenOpts.hostname,
      port: listenOpts.port,
      cert: listenOpts.cert,
      key: listenOpts.key,
      reusePort: listenOpts.reusePort,
    });
  } else {
    listener = listen({
      hostname: listenOpts.hostname,
      port: listenOpts.port,
      reusePort: listenOpts.reusePort,
    });
  }

  const serverDeferred = new Deferred();
  const activeHttpConnections = new SafeSet();

  const server = {
    transport: listenOpts.cert && listenOpts.key ? "https" : "http",
    hostname: listenOpts.hostname,
    port: listenOpts.port,
    closed: false,

    close() {
      if (server.closed) {
        return;
      }
      server.closed = true;
      try {
        listener.close();
      } catch {
        // Might have been already closed.
      }

      for (const httpConn of new SafeSetIterator(activeHttpConnections)) {
        try {
          httpConn.close();
        } catch {
          // Might have been already closed.
        }
      }

      SetPrototypeClear(activeHttpConnections);
      serverDeferred.resolve();
    },

    async serve() {
      while (!server.closed) {
        let conn;

        try {
          conn = await listener.accept();
        } catch {
          // Listener has been closed.
          if (!server.closed) {
            console.log("Listener has closed unexpectedly");
          }
          break;
        }

        let httpConn;
        try {
          const rid = ops.op_http_start(conn.rid);
          httpConn = new HttpConn(rid, conn.remoteAddr, conn.localAddr);
        } catch {
          // Connection has been closed;
          continue;
        }

        SetPrototypeAdd(activeHttpConnections, httpConn);

        const connInfo = {
          localAddr: conn.localAddr,
          remoteAddr: conn.remoteAddr,
        };
        // Serve the HTTP connection
        serveConnection(
          server,
          activeHttpConnections,
          handler,
          httpConn,
          connInfo,
          onError,
        );
      }
      await serverDeferred.promise;
    },
  };

  signal?.addEventListener(
    "abort",
    () => {
      try {
        server.close();
      } catch {
        // Pass
      }
    },
    { once: true },
  );

  onListen(listener.addr);

  await PromisePrototypeCatch(server.serve(), console.error);
}

export { _ws, HttpConn, serve, upgradeHttp, upgradeHttpRaw, upgradeWebSocket };
