// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const webidl = window.__bootstrap.webidl;
  const { InnerBody } = window.__bootstrap.fetchBody;
  const { setEventTargetData } = window.__bootstrap.eventTarget;
  const {
    Response,
    fromInnerRequest,
    toInnerResponse,
    newInnerRequest,
    newInnerResponse,
    fromInnerResponse,
  } = window.__bootstrap.fetch;
  const core = window.Deno.core;
  const { BadResource, Interrupted } = core;
  const { ReadableStream } = window.__bootstrap.streams;
  const abortSignal = window.__bootstrap.abortSignal;
  const { WebSocket, _rid, _readyState, _eventLoop, _protocol, _server } =
    window.__bootstrap.webSocket;
  const {
    ArrayPrototypeIncludes,
    ArrayPrototypePush,
    ArrayPrototypeSome,
    Promise,
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
  } = window.__bootstrap.primordials;

  const connErrorSymbol = Symbol("connError");

  class HttpConn {
    #rid = 0;
    // This set holds resource ids of resources
    // that were created during lifecycle of this request.
    // When the connection is closed these resources should be closed
    // as well.
    managedResources = new Set();

    constructor(rid) {
      this.#rid = rid;
    }

    /** @returns {number} */
    get rid() {
      return this.#rid;
    }

    /** @returns {Promise<ResponseEvent | null>} */
    async nextRequest() {
      let nextRequest;
      try {
        nextRequest = await core.opAsync(
          "op_http_request_next",
          this.#rid,
        );
      } catch (error) {
        // A connection error seen here would cause disrupted responses to throw
        // a generic `BadResource` error. Instead store this error and replace
        // those with it.
        this[connErrorSymbol] = error;
        if (error instanceof BadResource) {
          return null;
        } else if (error instanceof Interrupted) {
          return null;
        } else if (
          StringPrototypeIncludes(error.message, "connection closed")
        ) {
          return null;
        }
        throw error;
      }
      if (nextRequest === null) return null;

      const [
        requestRid,
        responseSenderRid,
        method,
        headersList,
        url,
      ] = nextRequest;

      /** @type {ReadableStream<Uint8Array> | undefined} */
      let body = null;
      if (typeof requestRid === "number") {
        SetPrototypeAdd(this.managedResources, requestRid);
        body = createRequestBodyStream(this, requestRid);
      }

      const innerRequest = newInnerRequest(
        method,
        url,
        headersList,
        body !== null ? new InnerBody(body) : null,
      );
      const signal = abortSignal.newSignal();
      const request = fromInnerRequest(innerRequest, signal, "immutable");

      SetPrototypeAdd(this.managedResources, responseSenderRid);
      const respondWith = createRespondWith(
        this,
        responseSenderRid,
        requestRid,
      );

      return { request, respondWith };
    }

    /** @returns {void} */
    close() {
      for (const rid of SetPrototypeValues(this.managedResources)) {
        try {
          core.close(rid);
        } catch (_e) {
          // pass, might have already been closed
        }
      }
      core.close(this.#rid);
    }

    [SymbolAsyncIterator]() {
      // deno-lint-ignore no-this-alias
      const httpConn = this;
      return {
        async next() {
          const reqEvt = await httpConn.nextRequest();
          // Change with caution, current form avoids a v8 deopt
          return { value: reqEvt, done: reqEvt === null };
        },
      };
    }
  }

  function readRequest(requestRid, zeroCopyBuf) {
    return core.opAsync(
      "op_http_request_read",
      requestRid,
      zeroCopyBuf,
    );
  }

  function createRespondWith(httpConn, responseSenderRid, requestRid) {
    return async function respondWith(resp) {
      if (resp instanceof Promise) {
        resp = await resp;
      }

      if (!(resp instanceof Response)) {
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
        if (innerResp.body.unusable()) throw new TypeError("Body is unusable.");
        if (innerResp.body.streamOrStatic instanceof ReadableStream) {
          if (
            innerResp.body.length === null ||
            innerResp.body.source instanceof Blob
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

      SetPrototypeDelete(httpConn.managedResources, responseSenderRid);
      let responseBodyRid;
      try {
        responseBodyRid = await core.opAsync("op_http_response", [
          responseSenderRid,
          innerResp.status ?? 200,
          innerResp.headerList,
        ], respBody instanceof Uint8Array ? respBody : null);
      } catch (error) {
        const connError = httpConn[connErrorSymbol];
        if (error instanceof BadResource && connError != null) {
          // deno-lint-ignore no-ex-assign
          error = new connError.constructor(connError.message);
        }
        if (respBody !== null && respBody instanceof ReadableStream) {
          await respBody.cancel(error);
        }
        throw error;
      }

      // If `respond` returns a responseBodyRid, we should stream the body
      // to that resource.
      if (responseBodyRid !== null) {
        SetPrototypeAdd(httpConn.managedResources, responseBodyRid);
        try {
          if (respBody === null || !(respBody instanceof ReadableStream)) {
            throw new TypeError("Unreachable");
          }
          const reader = respBody.getReader();
          while (true) {
            const { value, done } = await reader.read();
            if (done) break;
            if (!(value instanceof Uint8Array)) {
              await reader.cancel(new TypeError("Value not a Uint8Array"));
              break;
            }
            try {
              await core.opAsync(
                "op_http_response_write",
                responseBodyRid,
                value,
              );
            } catch (error) {
              const connError = httpConn[connErrorSymbol];
              if (error instanceof BadResource && connError != null) {
                // deno-lint-ignore no-ex-assign
                error = new connError.constructor(connError.message);
              }
              await reader.cancel(error);
              throw error;
            }
          }
        } finally {
          // Once all chunks are sent, and the request body is closed, we can
          // close the response body.
          SetPrototypeDelete(httpConn.managedResources, responseBodyRid);
          try {
            await core.opAsync("op_http_response_close", responseBodyRid);
          } catch { /* pass */ }
        }
      }

      const ws = resp[_ws];
      if (ws) {
        if (typeof requestRid !== "number") {
          throw new TypeError(
            "This request can not be upgraded to a websocket connection.",
          );
        }

        const wsRid = await core.opAsync(
          "op_http_upgrade_websocket",
          requestRid,
        );
        ws[_rid] = wsRid;
        ws[_protocol] = resp.headers.get("sec-websocket-protocol");

        if (ws[_readyState] === WebSocket.CLOSING) {
          await core.opAsync("op_ws_close", { rid: wsRid });

          ws[_readyState] = WebSocket.CLOSED;

          const errEvent = new ErrorEvent("error");
          ws.dispatchEvent(errEvent);

          const event = new CloseEvent("close");
          ws.dispatchEvent(event);

          try {
            core.close(wsRid);
          } catch (err) {
            // Ignore error if the socket has already been closed.
            if (!(err instanceof Deno.errors.BadResource)) throw err;
          }
        } else {
          ws[_readyState] = WebSocket.OPEN;
          const event = new Event("open");
          ws.dispatchEvent(event);

          ws[_eventLoop]();
        }
      }
    };
  }

  function createRequestBodyStream(httpConn, requestRid) {
    return new ReadableStream({
      type: "bytes",
      async pull(controller) {
        try {
          // This is the largest possible size for a single packet on a TLS
          // stream.
          const chunk = new Uint8Array(16 * 1024 + 256);
          const read = await readRequest(
            requestRid,
            chunk,
          );
          if (read > 0) {
            // We read some data. Enqueue it onto the stream.
            controller.enqueue(TypedArrayPrototypeSubarray(chunk, 0, read));
          } else {
            // We have reached the end of the body, so we close the stream.
            controller.close();
            SetPrototypeDelete(httpConn.managedResources, requestRid);
            core.close(requestRid);
          }
        } catch (err) {
          // There was an error while reading a chunk of the body, so we
          // error.
          controller.error(err);
          controller.close();
          SetPrototypeDelete(httpConn.managedResources, requestRid);
          core.close(requestRid);
        }
      },
      cancel() {
        SetPrototypeDelete(httpConn.managedResources, requestRid);
        core.close(requestRid);
      },
    });
  }

  const _ws = Symbol("[[associated_ws]]");

  function upgradeWebSocket(request, options = {}) {
    if (request.headers.get("upgrade") !== "websocket") {
      throw new TypeError(
        "Invalid Header: 'upgrade' header must be 'websocket'",
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
        "Invalid Header: 'connection' header must be 'Upgrade'",
      );
    }

    const websocketKey = request.headers.get("sec-websocket-key");
    if (websocketKey === null) {
      throw new TypeError(
        "Invalid Header: 'sec-websocket-key' header must be set",
      );
    }

    const accept = core.opSync("op_http_websocket_accept_header", websocketKey);

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

    return { response, socket };
  }

  window.__bootstrap.http = {
    HttpConn,
    upgradeWebSocket,
  };
})(this);
