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
  const { BadResourcePrototype, InterruptedPrototype } = core;
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

  class Request {
    #token;
    #method;
    #url;
    #headers;

    constructor(token) {
      this.#token = token;
    }

    get method() {
      if (!this.#method) {
        this.#method = core.op_method(this.#token);
      }
      return this.#method;
    }
    get url() {
      if (!this.#url) {
        this.#url = core.op_path(this.#token);
      }
      return this.#url;
    }
    get headers() {
      if (!this.#headers) {
        this.#headers = core.op_headers(this.#token);
      }
      return this.#headers;
    }
  }

  class Response {
    head;
    body = null;
    isStream = false;
    constructor(body, init) {
      this.head = `HTTP/1.1 ${init?.statusCode ?? 200} ${
        init?.statusText ?? "OK"
      }\r\n`;
      // Write raw headers in JS to avoid serializing overhead.
      if (init?.headers) {
        this.head += init.headers.map((h) => h.join(": ")).join("\r\n");
        this.head += "\r\n";
      }
      // body can be a
      // 1. string / Uint8Array / ArrayBuffer (static).
      // 2. number (stream). TODO(@littledivy): will use ReadableStream[_maybeRid] here
      //
      // * Uint8Array (and maybe number) will take the fast call path.
      // * number is take the async op, it can be a fast api too.
      // * string will be appended to the head, leaving body null.
      // * hopefully ArrayBuffer will take the fast path too (waiting for V8 support).
      if (typeof body === "string") {
        this.head += `Content-length: ${body.length}\r\n\r\n${body}`;
      } else if (typeof body === "number") {
        this.isStream = true;
        this.body = body;
      } else {
        this.body = body;
      }
    }

    static json(data, init = {}) {
      if (init.headers === undefined) {
        init.headers = [];
      }
      init.headers.push(["Content-Type", "application/json"]);
      return new Response(JSON.stringify(data), init);
    }
    static redirect(url, status = 302) {}
  }

  class HttpConn {
    #listener;

    constructor() {
      this.#listener = core.opAsync("op_listen");
    }

    async *[Symbol.asyncIterator]() {
      while (true) {
        const token = await core.opAsync("op_next");
        for (let i = 0; i < token; i++) {
          const request = new Request(i);
          async function respondWith(resp) {
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

            core.ops.op_respond(
              i,
              innerResp.status ?? 200,
              innerResp.headerList,
              isStreamingResponseBody ? null : respBody,
            );
            return;
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
                  if (
                    !ObjectPrototypeIsPrototypeOf(Uint8ArrayPrototype, value)
                  ) {
                    await reader.cancel(
                      new TypeError("Value not a Uint8Array"),
                    );
                    break;
                  }
                  try {
                    await core.opAsync("op_http_write", streamRid, value);
                  } catch (error) {
                    const connError = httpConn[connErrorSymbol];
                    if (
                      ObjectPrototypeIsPrototypeOf(
                        BadResourcePrototype,
                        error,
                      ) &&
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

            // const deferred = request[_deferred];
            // if (deferred) {
            //   const res = await core.opAsync("op_http_upgrade", streamRid);
            //   let conn;
            //   if (res.connType === "tcp") {
            //     conn = new TcpConn(res.connRid, remoteAddr, localAddr);
            //   } else if (res.connType === "tls") {
            //     conn = new TlsConn(res.connRid, remoteAddr, localAddr);
            //   } else if (res.connType === "unix") {
            //     conn = new UnixConn(res.connRid, remoteAddr, localAddr);
            //   } else {
            //     throw new Error("unreachable");
            //   }

            //   deferred.resolve([conn, res.readBuf]);
            // }
            // const ws = resp[_ws];
            // if (ws) {
            //   const wsRid = await core.opAsync(
            //     "op_http_upgrade_websocket",
            //     streamRid,
            //   );
            //   ws[_rid] = wsRid;
            //   ws[_protocol] = resp.headers.get("sec-websocket-protocol");

            //   httpConn.close();

            //   ws[_readyState] = WebSocket.OPEN;
            //   const event = new Event("open");
            //   ws.dispatchEvent(event);

            //   ws[_eventLoop]();
            //   if (ws[_idleTimeoutDuration]) {
            //     ws.addEventListener(
            //       "close",
            //       () => clearTimeout(ws[_idleTimeoutTimeout]),
            //     );
            //   }
            //   ws[_serverHandleIdleTimeout]();
            // }
          }
          yield { request, respondWith };
        }
      }
    }
  }

  async function serve(handler, opts) {
    const listener = core.opAsync(
      "op_listen",
      opts,
    );
    while (true) {
      const token = await core.opAsync("op_next");
      for (let i = 0; i < token; i++) {
        const req = new Request(i);
        const res = handler(req);
        core.ops.op_respond(i, res.head, res.body);
      }
    }
    await listener;
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
                isStreamingResponseBody ? null : respBody,
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

  window.__bootstrap.flash = {
    HttpConn,
    Response,
    serve,
  };
})(this);
