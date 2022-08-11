// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const { BlobPrototype } = window.__bootstrap.file;
  const {
    fromInnerFlashRequest,
    toInnerResponse,
  } = window.__bootstrap.fetch;
  const core = window.Deno.core;
  const { TcpConn } = window.__bootstrap.net;
  const { ReadableStream, ReadableStreamPrototype, _state } =
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

  let dateInterval;
  let date;

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
    // Date header: https://datatracker.ietf.org/doc/html/rfc7231#section-7.1.1.2
    let str = `HTTP/1.1 ${status} ${statusCodes[status]}\r\nDate: ${date}\r\n`;
    for (const [name, value] of headerList) {
      // header-field   = field-name ":" OWS field-value OWS
      str += `${name}: ${value}\r\n`;
    }

    // TODO(@littledivy): MUST generate an Upgrade header field in a 426 response. https://datatracker.ietf.org/doc/html/rfc7231#section-6.5.15
    // TODO(@littledivy): MUST generate an Allow header field in a 405 response containing a list of the target
    //      resource's currently supported methods.
    // TODD: Don't send body for HEAD requests.
    // null body status is validated by inititalizeAResponse in ext/fetch
    // * MUST NOT generate a payload in a 205 response https://datatracker.ietf.org/doc/html/rfc7231#section-6.3.6
    // * MUST NOT send Content-Length if status code is 1xx or 204.
    // * MUST NOT send Content-Length if status code is 2xx to a CONNECT request
    if (body) {
      str += `Content-Length: ${body?.length}\r\n\r\n`;
    } else {
      // TODO(littledivy): support compression.
      // TODO(littledivy): MUST NOT send transfer-encoding if:
      //   * status code is 1xx or 204
      //   * status code is 2xx to a CONNECT request
      //   * request indicates HTTP/1.1
      str += "Transfer-Encoding: chunked\r\n\r\n";
    }

    return str + (body ?? "");
  }

  async function serve(handler, opts = {}) {
    opts = { hostname: "127.0.0.1", port: 9000, ...opts };
    const signal = opts.signal;
    delete opts.signal;
    const serverId = core.ops.op_flash_serve(opts);
    const serverPromise = core.opAsync("op_flash_drive_server", serverId);

    const server = {
      id: serverId,
      transport: opts.cert && opts.key ? "https" : "http",
      hostname: opts.hostname,
      port: opts.port,
      closed: false,
      finished: (async () => {
        return await serverPromise;
      })(),
      async close() {
        if (server.closed) {
          return;
        }
        server.closed = true;
        await core.opAsync("op_flash_close_server", serverId);
        await server.finished;
      },
      async serve() {
        while (true) {
          if (server.closed) {
            break;
          }

          let token = nextRequestSync();
          if (token === 0) {
            token = await core.opAsync("op_flash_next_async", serverId);
            if (server.closed) {
              break;
            }
          }

          for (let i = 0; i < token; i++) {
            let body = null;
            // There might be a body, but we don't expose it for GET/HEAD requests.
            // It will be closed automatically once the request has been handled and
            // the response has been sent.
            // TODO: mask into the token maybe?
            const hasBody = core.ops.op_flash_has_body_stream(serverId, i);
            if (hasBody) {
              body = createRequestBodyStream(serverId, i);
            }

            const req = fromInnerFlashRequest(
              serverId,
              /* streamRid */
              i,
              body,
              /* methodCb */
              () => core.ops.op_flash_method(serverId, i),
              /* urlCb */
              () => {
                const path = core.ops.op_flash_path(serverId, i);
                return `${server.transport}://${server.hostname}:${server.port}${path}`;
              },
              /* headersCb */
              () => core.opSync("op_flash_headers", serverId, i),
            );

            const resp = await handler(req);
            // there might've been an HTTP upgrade.
            if (resp === undefined) {
              continue;
            }
            if (hasBody && body[_state] !== "closed") {
              // TODO(@littledivy): Optimize by draining in a single op.
              await req.arrayBuffer();
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

            const ws = resp[_ws];
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
                !ws, // Don't close socket if there is a deferred websocket upgrade.
              );
            }

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
        await server.finished;
      },
    };

    signal?.addEventListener("abort", () => {
      clearInterval(dateInterval);
      server.close().then(() => {}, () => {});
    }, {
      once: true,
    });

    // NOTE(bartlomieju): this drives the server
    (async () => {
      await server.finished;
    });

    let nextRequestSync = core.ops.op_flash_next;
    if (serverId > 0) {
      nextRequestSync = () => core.ops.op_flash_next_server(serverId);
    }

    if (!dateInterval) {
      dateInterval = setInterval(() => {
        date = new Date().toUTCString();
      }, 1000);
    }

    return await server.serve();
  }

  function createRequestBodyStream(serverId, token) {
    // The first packet is left over bytes after parsing the request
    const firstRead = core.ops.op_flash_first_packet(
      serverId,
      token,
    );
    let firstEnqueued = firstRead.byteLength == 0;

    return new ReadableStream({
      type: "bytes",
      async pull(controller) {
        try {
          if (firstEnqueued === false) {
            controller.enqueue(firstRead);
            firstEnqueued = true;
            return;
          }
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

  window.__bootstrap.flash = {
    serve,
  };
})(this);
