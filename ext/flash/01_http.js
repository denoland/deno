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
    fromInnerFlashRequest,
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
      return `http://localhost:9000${this.#url}`;
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

    async arrayBuffer() {
      let u8 = new Uint8Array(1024 * 1024);
      let written = 0;
      while (true) {
        // TODO(@littledivy): This can be a fast api call.
        const n = await core.opAsync("op_read_body", this.#token, u8);
        if (n === 0) {
          break;
        }
        written += n;
      }
      return u8.subarray(0, written).buffer;
    }

    async text() {
      return core.decode(await this.arrayBuffer());
    }

    async json() {
      return JSON.parse((await this.text()).trim());
    }

    blob() {}
    formData() {}
    get bodyUsed() {}

    #stream;
    get body() {
      if (!this.#stream) {
        this.#stream = createRequestBodyStream(this.#token);
      }
      return this.#stream;
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

  async function serve(handler, opts) {
    const listener = core.opAsync(
      "op_listen",
      opts,
    );
    while (true) {
      let token = core.ops.op_next();
      if (token === 0) token = await core.opAsync("op_next_async");
      for (let i = 0; i < token; i++) {
        const req = fromInnerFlashRequest(
          createRequestBodyStream(i),
          () => core.ops.op_method(i),
          () => core.ops.op_path(i),
          () =>
          headersFromHeaderList(
            core.ops.op_headers(i),
            "request",
          ));
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
          a:
          while (true) {
            const { value, done } = await reader.read();
            if (first) {
              first = false;
              core.ops.op_respond(
                i,
                http1Response(
                  innerResp.status ?? 200,
                  innerResp.headerList,
                  null,
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

  function createRequestBodyStream(token) {
    return new ReadableStream({
      type: "bytes",
      async pull(controller) {
        try {
          // This is the largest possible size for a single packet on a TLS
          // stream.
          const chunk = new Uint8Array(16 * 1024 + 256);
          const read = await core.opAsync("op_read_body", token, chunk);

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

  window.__bootstrap.flash = {
    serve,
  };
})(this);
