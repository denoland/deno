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
  const { _ws } = window.__bootstrap.http;
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
  const { headersFromHeaderList } = window.__bootstrap.headers;

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
        this.#method = core.ops.op_method(this.#token);
      }
      return this.#method;
    }

    get url() {
      if (!this.#url) {
        this.#url = core.ops.op_path(this.#token);
      }
      return `http://localhost:9000`;
    }

    get headers() {
      if (!this.#headers) {
        this.#headers = headersFromHeaderList(
          core.ops.op_headers(this.#token),
          "request",
        );
      }
      return this.#headers;
    }
  }

  const methods = {
    200: "OK",
  };

  function http1Response(status, headerList, body) {
    let str = `HTTP/1.1 ${status} ${methods[status]}\r\n`;
    for (const [name, value] of headerList) {
      str += `${name}: ${value}\r\n`;
    }
    if (body) {
      str += `Content-Length: ${body?.length}\r\n\r\n`;
    } else {
      str += "Transfer-Encoding: chunked\r\n\r\n";
    }
    return str + (body ?? "");
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
            let isStreamingResponseBody = false;
            if (innerResp.body !== null) {
              if (typeof innerResp.body.streamOrStatic?.body === "string") {
                if (innerResp.body.streamOrStatic.consumed === true) {
                  throw new TypeError("Body is unusable.");
                }
                innerResp.body.streamOrStatic.consumed = true;
                respBody = innerResp.body.streamOrStatic.body;
                isStreamingResponseBody = false;
              } else if (
                ObjectPrototypeIsPrototypeOf(
                  ReadableStreamPrototype,
                  innerResp.body.streamOrStatic,
                )
              ) {
                if (innerResp.body.unusable()) {
                  throw new TypeError("Body is unusable.");
                }
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
                isStreamingResponseBody = !(
                  typeof respBody === "string" ||
                  ObjectPrototypeIsPrototypeOf(Uint8ArrayPrototype, respBody)
                );
              } else {
                if (innerResp.body.streamOrStatic.consumed === true) {
                  throw new TypeError("Body is unusable.");
                }
                innerResp.body.streamOrStatic.consumed = true;
                respBody = innerResp.body.streamOrStatic.body;
              }
            } else {
              respBody = new Uint8Array(0);
            }

            const ws = resp[_ws];
            core.ops.op_respond(
              i,
              http1Response(
                innerResp.status ?? 200,
                innerResp.headerList,
                isStreamingResponseBody ? null : respBody,
              ),
              !ws,
            );

            if (isStreamingResponseBody === true) {
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
                    "op_write_stream",
                    i,
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

            if (ws) {
              const wsRid = await core.opAsync(
                "op_upgrade_websocket",
                i,
              );

              ws[_rid] = wsRid;
              ws[_protocol] = resp.headers.get("sec-websocket-protocol");

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
        const resp = await handler(req);
        const innerResp = toInnerResponse(resp);

        // If response body length is known, it will be sent synchronously in a
        // single op, in other case a "response body" resource will be created and
        // we'll be streaming it.
        /** @type {ReadableStream<Uint8Array> | Uint8Array | null} */
        let respBody = null;
        let isStreamingResponseBody = false;
        if (innerResp.body !== null) {
          if (typeof innerResp.body.streamOrStatic?.body === "string") {
            if (innerResp.body.streamOrStatic.consumed === true) {
              throw new TypeError("Body is unusable.");
            }
            innerResp.body.streamOrStatic.consumed = true;
            respBody = innerResp.body.streamOrStatic.body;
            isStreamingResponseBody = false;
          } else if (
            ObjectPrototypeIsPrototypeOf(
              ReadableStreamPrototype,
              innerResp.body.streamOrStatic,
            )
          ) {
            if (innerResp.body.unusable()) {
              throw new TypeError("Body is unusable.");
            }
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
            isStreamingResponseBody = !(
              typeof respBody === "string" ||
              ObjectPrototypeIsPrototypeOf(Uint8ArrayPrototype, respBody)
            );
          } else {
            if (innerResp.body.streamOrStatic.consumed === true) {
              throw new TypeError("Body is unusable.");
            }
            innerResp.body.streamOrStatic.consumed = true;
            respBody = innerResp.body.streamOrStatic.body;
          }
        } else {
          respBody = new Uint8Array(0);
        }

        const ws = resp[_ws];
        if (isStreamingResponseBody === true) {
          // const resourceRid = getReadableStreamRid(respBody);
          let reader = respBody.getReader();
          let first = true;
          a: while (true) {
            const { value, done } = await reader.read();
            if (first) {
              first = false;
              core.ops.op_respond(
                i,
                http1Response(
                  innerResp.status ?? 200,
                  innerResp.headerList,
                  null
                ),
                value,
                false,
              );
            } else {
              core.ops.op_respond_chuncked(
                i,
                value,
                done,
              );
            }
            if (done) break a;
          }
        } else {
          core.ops.op_respond(
            i,
            http1Response(
              innerResp.status ?? 200,
              innerResp.headerList,
              respBody,
            ),
            null,
            false,
          );
        }
      }
    }
    await listener;
  }

  function readRequest(streamRid, buf) {
    return core.opAsync("op_http_read", streamRid, buf);
  }

  window.__bootstrap.flash = {
    HttpConn,
    serve,
  };
})(this);
