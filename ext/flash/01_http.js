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

  const statusCodes = {
    100: "Continue",
    101: "Switching Protocols",
    102: "Processing",
    200: "OK",
    201: "Created",
    202: "Accepted",
    203: "Non Authoritative Information",
    204: "No Content",
    205: "Reset Content",
    206: "Partial Content",
    207: "Multi-Status",
    208: "Already Reported",
    226: "IM Used",
    300: "Multiple Choices",
    301: "Moved Permanently",
    302: "Found",
    303: "See Other",
    304: "Not Modified",
    305: "Use Proxy",
    307: "Temporary Redirect",
    308: "Permanent Redirect",
    400: "Bad Request",
    401: "Unauthorized",
    402: "Payment Required",
    403: "Forbidden",
    404: "Not Found",
    405: "Method Not Allowed",
    406: "Not Acceptable",
    407: "Proxy Authentication Required",
    408: "Request Timeout",
    409: "Conflict",
    410: "Gone",
    411: "Length Required",
    412: "Precondition Failed",
    413: "Payload Too Large",
    414: "URI Too Long",
    415: "Unsupported Media Type",
    416: "Range Not Satisfiable",
    418: "I'm a teapot",
    421: "Misdirected Request",
    422: "Unprocessable Entity",
    423: "Locked",
    424: "Failed Dependency",
    426: "Upgrade Required",
    428: "Precondition Required",
    429: "Too Many Requests",
    431: "Request Header Fields Too Large",
    451: "Unavailable For Legal Reasons",
    500: "Internal Server Error",
    501: "Not Implemented",
    502: "Bad Gateway",
    503: "Service Unavailable",
    504: "Gateway Timeout",
    505: "HTTP Version Not Supported",
    506: "Variant Also Negotiates",
    507: "Insufficient Storage",
    508: "Loop Detected",
    510: "Not Extended",
    511: "Network Authentication Required",
  };

  // Construct an HTTP response message.
  // All HTTP/1.1 messages consist of a start-line followed by a sequence
  // of octets.
  //
  //  HTTP-message = start-line
  //    *( header-field CRLF )
  //    CRLF
  //    [ message-body ]
  //
  function http1Response(status, headerList, body) {
    // HTTP uses a "<major>.<minor>" numbering scheme
    //   HTTP-version  = HTTP-name "/" DIGIT "." DIGIT
    //   HTTP-name     = %x48.54.54.50 ; "HTTP", case-sensitive
    //
    // status-line = HTTP-version SP status-code SP reason-phrase CRLF
    let str = `HTTP/1.1 ${status} ${statusCodes[status]}\r\n`;
    for (const [name, value] of headerList) {
      // header-field   = field-name ":" OWS field-value OWS
      str += `${name}: ${value}\r\n`;
    }
    if (body) {
      // TODO(littledivy): If status code == 304, MUST NOT send Content-Length if body length equals length that
      // would have been sent in the payload body of a response if the same request had used the GET method.
      // TODO(littledivy): MUST NOT send Content-Length if status code is 1xx or 204.
      // TODO(littledivy): MUST NOT send Content-Length if status code is 2xx to a CONNECT request
      str += `Content-Length: ${body?.length}\r\n\r\n`;
    } else {
      // TODO(littledivy): support compression.
      // TODO(littledivy): MUST NOT send transfer-encoding if:
      //   * status code is 1xx or 204
      //   * status code is 2xx to a CONNECT request
      //   * request indicates HTTP/1.1
      str += "Transfer-Encoding: chunked\r\n\r\n";
    }
    // TOOD: Don't send body for HEAD requests
    return str + (body ?? "");
  }

  async function serve(handler, opts) {
    const serverId = core.ops.op_flash_serve(
      { hostname: "127.0.0.1", port: 9000, ...opts },
    );
    const serverPromise = core.opAsync("op_flash_drive_server", serverId);

    const server = {
      id: serverId,
      serverPromise,
    };

    (async () => {
      await server.serverPromise;
    });

    let nextRequestSync = core.ops.op_flash_next;
    if (serverId > 0) {
      nextRequestSync = () => core.ops.op_flash_next_server(serverId);
    }

    while (true) {
      let token = nextRequestSync();
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
        // there might've been an HTTP upgrade.
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
