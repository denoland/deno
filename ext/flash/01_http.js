// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const { BlobPrototype } = window.__bootstrap.file;
  const {
    _flash,
    fromInnerFlashRequest,
    toInnerResponse,
  } = window.__bootstrap.fetch;
  const core = window.Deno.core;
  const { TcpConn } = window.__bootstrap.net;
  const { ReadableStream, ReadableStreamPrototype } =
    window.__bootstrap.streams;
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
  const { _ws } = window.__bootstrap.http;
  const {
    ObjectPrototypeIsPrototypeOf,
    TypedArrayPrototypeSubarray,
    TypeError,
    Uint8Array,
    Uint8ArrayPrototype,
  } = window.__bootstrap.primordials;
  const { headersFromHeaderList } = window.__bootstrap.headers;

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
      "op_flash_listen",
      { hostname: "127.0.0.1", port: 9000, ...opts },
    );
    // FIXME(bartlomieju): should be a field on "listener"
    const serverId = 0;
    while (true) {
      let token = core.ops.op_flash_next(serverId);
      if (token === 0) {
        token = await core.opAsync("op_flash_next_async", serverId);
      }
      for (let i = 0; i < token; i++) {
        const req = fromInnerFlashRequest(
          null,
          // createRequestBodyStream(serverId, i),
          () => core.ops.op_flash_method(serverId, i),
          () => core.ops.op_flash_path(serverId, i),
          () =>
            headersFromHeaderList(
              core.ops.op_flash_headers(serverId, i),
              "request",
            ),
          i,
        );

        const resp = await handler(req);
        // Probably an http connect upgrade
        if (resp === undefined) {
          continue;
        }
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

        if (isStreamingResponseBody === true) {
          // const resourceRid = getReadableStreamRid(respBody);
          const reader = respBody.getReader();
          let first = true;
          a:
          while (true) {
            const { value, done } = await reader.read();
            if (first) {
              first = false;
              core.ops.op_flash_respond(
                serverId,
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
              core.ops.op_flash_respond_chuncked(
                serverId,
                i,
                value,
                done,
              );
            }
            if (done) break a;
          }
        } else {
          core.ops.op_flash_respond(
            serverId,
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

        const ws = resp[_ws];
        if (ws) {
          const wsRid = await core.opAsync(
            "op_flash_upgrade_websocket",
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
    }
    // deno-lint-ignore no-unreachable
    await listener;
  }

  // deno-lint-ignore no-unused-vars
  function createRequestBodyStream(serverId, token) {
    return new ReadableStream({
      type: "bytes",
      async pull(controller) {
        try {
          // This is the largest possible size for a single packet on a TLS
          // stream.
          const chunk = new Uint8Array(16 * 1024 + 256);
          const read = await core.opAsync(
            "op_flash_read_body",
            serverId,
            token,
            chunk,
          );
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

  function upgradeHttp(req) {
    const { streamRid } = req[_flash];
    const connRid = core.ops.op_flash_upgrade_http(streamRid);
    // TODO(@littledivy): return already read first packet too.
    return [new TcpConn(connRid), new Uint8Array()];
  }

  window.__bootstrap.flash = {
    serve,
    upgradeHttp,
  };
})(this);
