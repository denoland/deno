// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const webidl = window.__bootstrap.webidl;
  const { forgivingBase64Encode } = window.__bootstrap.infra;
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
  const errors = window.__bootstrap.errors.errors;
  const core = window.Deno.core;
  const { ReadableStream } = window.__bootstrap.streams;

  function serveHttp(conn) {
    const rid = Deno.core.opSync("op_http_start", conn.rid);
    return new HttpConn(rid);
  }

  const connErrorSymbol = Symbol("connError");

  class HttpConn {
    #rid = 0;

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
        nextRequest = await Deno.core.opAsync(
          "op_http_request_next",
          this.#rid,
        );
      } catch (error) {
        // A connection error seen here would cause disrupted responses to throw
        // a generic `BadResource` error. Instead store this error and replace
        // those with it.
        this[connErrorSymbol] = error;
        if (error instanceof errors.BadResource) {
          return null;
        } else if (error instanceof errors.Interrupted) {
          return null;
        } else if (error.message.includes("connection closed")) {
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
        body = createRequestBodyStream(requestRid);
      }

      const innerRequest = newInnerRequest(
        method,
        url,
        headersList,
        body !== null ? new InnerBody(body) : null,
      );
      const request = fromInnerRequest(innerRequest, null, "immutable");

      const respondWith = createRespondWith(
        this,
        responseSenderRid,
        requestRid,
      );

      return { request, respondWith };
    }

    /** @returns {void} */
    close() {
      core.close(this.#rid);
    }

    [Symbol.asyncIterator]() {
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
    return Deno.core.opAsync(
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
          if (innerResp.body.length === null) {
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

      let responseBodyRid;
      try {
        responseBodyRid = await Deno.core.opAsync("op_http_response", [
          responseSenderRid,
          innerResp.status ?? 200,
          innerResp.headerList,
        ], respBody instanceof Uint8Array ? respBody : null);
      } catch (error) {
        const connError = httpConn[connErrorSymbol];
        if (error instanceof errors.BadResource && connError != null) {
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
              await Deno.core.opAsync(
                "op_http_response_write",
                responseBodyRid,
                value,
              );
            } catch (error) {
              const connError = httpConn[connErrorSymbol];
              if (error instanceof errors.BadResource && connError != null) {
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
          try {
            await Deno.core.opAsync("op_http_response_close", responseBodyRid);
          } catch { /* pass */ }
        }
      }

      const ws = resp[_ws];
      if (ws) {
        const _readyState = Symbol.for("[[readyState]]");

        if (typeof requestRid !== "number") {
          throw new TypeError(
            "This request can not be upgraded to a websocket connection.",
          );
        }

        const wsRid = await core.opAsync("op_http_upgrade_websocket", requestRid);
        ws[Symbol.for("[[rid]]")] = wsRid;
        // TODO: protocols & extensions

        if (ws[_readyState] === WebSocket.CLOSING) {
          core.opAsync("op_ws_close", {
            rid: wsRid,
          }).then(() => {
            ws[_readyState] = WebSocket.CLOSED;

            const errEvent = new ErrorEvent("error");
            errEvent.target = ws;
            ws.dispatchEvent(errEvent);

            const event = new CloseEvent("close");
            event.target = ws;
            ws.dispatchEvent(event);

            try {
              core.close(rid);
            } catch (err) {
              // Ignore error if the socket has already been closed.
              if (!(err instanceof Deno.errors.BadResource)) throw err;
            }
          });
        } else {
          ws[_readyState] = WebSocket.OPEN;
          const event = new Event("open");
          event.target = ws;
          ws.dispatchEvent(event);

          ws[Symbol.for("[[eventLoop]]")]();
        }
      }
    };
  }

  function createRequestBodyStream(requestRid) {
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
            controller.enqueue(chunk.subarray(0, read));
          } else {
            // We have reached the end of the body, so we close the stream.
            controller.close();
            core.close(requestRid);
          }
        } catch (err) {
          // There was an error while reading a chunk of the body, so we
          // error.
          controller.error(err);
          controller.close();
          core.close(requestRid);
        }
      },
      cancel() {
        core.close(requestRid);
      },
    });
  }

  const _ws = Symbol("[[associated_ws]]");

  async function upgradeWebSocket(request) {
    if (request.headers["Upgrade"] !== "websocket") {
      // Throw
    }

    if (request.headers["Connection"] !== "Upgrade") {
      // Throw
    }

    if (!request.headers["Sec-WebSocket-Key"]) {
      // Throw
    }

    const key = new TextEncoder().encode(
      request.headers["Sec-WebSocket-Key"] +
        "258EAFA5-E914-47DA-95CA-C5AB0DC85B11",
    );
    const accept = await crypto.subtle.digest("SHA-1", key);

    const r = newInnerResponse(101);
    r.headerList = [
      ["Upgrade", "websocket"],
      ["Connection", "Upgrade"],
      ["Sec-WebSocket-Accept", forgivingBase64Encode(new Uint8Array(accept))],
    ];

    const response = fromInnerResponse(r, "immutable");

    const websocket = webidl.createBranded(WebSocket);
    setEventTargetData(websocket);
    response[_ws] = websocket;

    return { response, websocket };
  }

  window.__bootstrap.http = {
    serveHttp,
    upgradeWebSocket,
  };
})(this);
