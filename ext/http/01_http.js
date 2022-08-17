// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const webidl = window.__bootstrap.webidl;
  const { InnerBody } = window.__bootstrap.fetchBody;
  const { setEventTargetData } = window.__bootstrap.eventTarget;
  const { BlobPrototype } = window.__bootstrap.file;
  const {
    ResponsePrototype,
    fromInnerRequest,
    toInnerResponse,
    newInnerRequest,
    newInnerResponse,
    fromInnerResponse,
  } = window.__bootstrap.fetch;
  const core = window.Deno.core;
  const { BadResourcePrototype, InterruptedPrototype, ops } = core;
  const { ReadableStream, ReadableStreamPrototype } =
    window.__bootstrap.streams;
  const abortSignal = window.__bootstrap.abortSignal;
  const {
    WebSocket,
    _rid,
    _readyState,
    _eventLoop,
    _protocol,
    _server,
    _idleTimeoutDuration,
    _idleTimeoutTimeout,
    _serverHandleIdleTimeout,
  } = window.__bootstrap.webSocket;
  const { TcpConn, UnixConn } = window.__bootstrap.net;
  const { TlsConn } = window.__bootstrap.tls;
  const { Deferred, getReadableStreamRid, readableStreamClose } =
    window.__bootstrap.streams;
  const {
    ArrayPrototypeIncludes,
    ArrayPrototypePush,
    ArrayPrototypeSome,
    Error,
    ObjectPrototypeIsPrototypeOf,
    Set,
    SetPrototypeAdd,
    SetPrototypeDelete,
    SetPrototypeValues,
    StringPrototypeIncludes,
    StringPrototypeToLowerCase,
    StringPrototypeSplit,
    Symbol,
    SymbolAsyncIterator,
    TypedArrayPrototypeSubarray,
    TypeError,
    Uint8Array,
    Uint8ArrayPrototype,
  } = window.__bootstrap.primordials;

  const connErrorSymbol = Symbol("connError");
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

      const [streamRid, method, url] = nextRequest;
      SetPrototypeAdd(this.managedResources, streamRid);

      /** @type {ReadableStream<Uint8Array> | undefined} */
      let body = null;
      // There might be a body, but we don't expose it for GET/HEAD requests.
      // It will be closed automatically once the request has been handled and
      // the response has been sent.
      if (method !== "GET" && method !== "HEAD") {
        body = createRequestBodyStream(streamRid);
      }

      const innerRequest = newInnerRequest(
        method,
        url,
        () => ops.op_http_headers(streamRid),
        body !== null ? new InnerBody(body) : null,
        false,
      );
      const signal = abortSignal.newSignal();
      const request = fromInnerRequest(innerRequest, signal, "immutable");

      const respondWith = createRespondWith(
        this,
        streamRid,
        request,
        this.#remoteAddr,
        this.#localAddr,
      );

      return { request, respondWith };
    }

    /** @returns {void} */
    close() {
      if (!this.#closed) {
        this.#closed = true;
        core.close(this.#rid);
        for (const rid of SetPrototypeValues(this.managedResources)) {
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

  function readRequest(streamRid, buf) {
    return core.opAsync("op_http_read", streamRid, buf);
  }

  function createRespondWith(
    httpConn,
    streamRid,
    request,
    remoteAddr,
    localAddr,
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
          if (
            respBody === null ||
            !ObjectPrototypeIsPrototypeOf(ReadableStreamPrototype, respBody)
          ) {
            throw new TypeError("Unreachable");
          }
          const resourceRid = getReadableStreamRid(respBody);
          let reader;
          if (resourceRid) {
            if (respBody.locked) {
              throw new TypeError("ReadableStream is locked.");
            }
            reader = respBody.getReader(); // Aquire JS lock.
            try {
              await core.opAsync(
                "op_http_write_resource",
                streamRid,
                resourceRid,
              );
              core.tryClose(resourceRid);
              readableStreamClose(respBody); // Release JS lock.
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
                await core.opAsync("op_http_write", streamRid, value);
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
          }

          try {
            await core.opAsync("op_http_shutdown", streamRid);
          } catch (error) {
            await reader.cancel(error);
            throw error;
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
      } finally {
        if (SetPrototypeDelete(httpConn.managedResources, streamRid)) {
          core.close(streamRid);
        }
      }
    };
  }

  function createRequestBodyStream(streamRid) {
    return new ReadableStream({
      type: "bytes",
      async pull(controller) {
        try {
          // This is the largest possible size for a single packet on a TLS
          // stream.
          const chunk = new Uint8Array(16 * 1024 + 256);
          const read = await readRequest(streamRid, chunk);
          if (read > 0) {
            // We read some data. Enqueue it onto the stream.
            controller.enqueue(TypedArrayPrototypeSubarray(chunk, 0, read));
          } else {
            // We have reached the end of the body, so we close the stream.
            controller.close();
          }
        } catch (err) {
          // There was an error while reading a chunk of the body, so we
          // error.
          controller.error(err);
          controller.close();
        }
      },
    });
  }

  const _ws = Symbol("[[associated_ws]]");

  function upgradeWebSocket(request, options = {}) {
    const upgrade = request.headers.get("upgrade");
    const upgradeHasWebSocketOption = upgrade !== null &&
      ArrayPrototypeSome(
        StringPrototypeSplit(upgrade, /\s*,\s*/),
        (option) => StringPrototypeToLowerCase(option) === "websocket",
      );
    if (!upgradeHasWebSocketOption) {
      throw new TypeError(
        "Invalid Header: 'upgrade' header must contain 'websocket'",
      );
    }

    const connection = request.headers.get("connection");
    const connectionHasUpgradeOption = connection !== null &&
      ArrayPrototypeSome(
        StringPrototypeSplit(connection, /\s*,\s*/),
        (option) => StringPrototypeToLowerCase(option) === "upgrade",
      );
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

  window.__bootstrap.http = {
    HttpConn,
    upgradeWebSocket,
    upgradeHttp,
  };
})(this);
