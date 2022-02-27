// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import {
  Buffer,
  BufReader,
  BufWriter,
} from "../../../test_util/std/io/buffer.ts";
import { TextProtoReader } from "../../../test_util/std/textproto/mod.ts";
import {
  assert,
  assertEquals,
  assertRejects,
  assertStrictEquals,
  assertThrows,
  deferred,
  delay,
  fail,
} from "./test_util.ts";

async function writeRequestAndReadResponse(conn: Deno.Conn): Promise<string> {
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();

  const w = new BufWriter(conn);
  const r = new BufReader(conn);
  const body = `GET / HTTP/1.1\r\nHost: 127.0.0.1:4501\r\n\r\n`;
  const writeResult = await w.write(encoder.encode(body));
  assertEquals(body.length, writeResult);
  await w.flush();
  const tpr = new TextProtoReader(r);
  const statusLine = await tpr.readLine();
  assert(statusLine !== null);
  const headers = await tpr.readMIMEHeader();
  assert(headers !== null);

  const chunkedReader = chunkedBodyReader(headers, r);
  const buf = new Uint8Array(5);
  const dest = new Buffer();
  let result: number | null;
  while ((result = await chunkedReader.read(buf)) !== null) {
    const len = Math.min(buf.byteLength, result);
    await dest.write(buf.subarray(0, len));
  }
  return decoder.decode(dest.bytes());
}

Deno.test({ permissions: { net: true } }, async function httpServerBasic() {
  const promise = (async () => {
    const listener = Deno.listen({ port: 4501 });
    for await (const conn of listener) {
      const httpConn = Deno.serveHttp(conn);
      for await (const { request, respondWith } of httpConn) {
        assertEquals(new URL(request.url).href, "http://127.0.0.1:4501/");
        assertEquals(await request.text(), "");
        respondWith(new Response("Hello World", { headers: { "foo": "bar" } }));
      }
      break;
    }
  })();

  const resp = await fetch("http://127.0.0.1:4501/", {
    headers: { "connection": "close" },
  });
  const clone = resp.clone();
  const text = await resp.text();
  assertEquals(text, "Hello World");
  assertEquals(resp.headers.get("foo"), "bar");
  const cloneText = await clone.text();
  assertEquals(cloneText, "Hello World");
  await promise;
});

Deno.test(
  { permissions: { net: true } },
  async function httpServerGetRequestBody() {
    const promise = (async () => {
      const listener = Deno.listen({ port: 4501 });
      const conn = await listener.accept();
      listener.close();
      const httpConn = Deno.serveHttp(conn);
      const e = await httpConn.nextRequest();
      assert(e);
      const { request, respondWith } = e;
      assertEquals(request.body, null);
      await respondWith(new Response("", { headers: {} }));
      httpConn.close();
    })();

    const conn = await Deno.connect({ port: 4501 });
    // Send GET request with a body + content-length.
    const encoder = new TextEncoder();
    const body =
      `GET / HTTP/1.1\r\nHost: 127.0.0.1:4501\r\nContent-Length: 5\r\n\r\n12345`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);

    const resp = new Uint8Array(200);
    const readResult = await conn.read(resp);
    assertEquals(readResult, 115);

    conn.close();

    await promise;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerStreamResponse() {
    const stream = new TransformStream();
    const writer = stream.writable.getWriter();
    writer.write(new TextEncoder().encode("hello "));
    writer.write(new TextEncoder().encode("world"));
    writer.close();

    const promise = (async () => {
      const listener = Deno.listen({ port: 4501 });
      const conn = await listener.accept();
      const httpConn = Deno.serveHttp(conn);
      const evt = await httpConn.nextRequest();
      assert(evt);
      const { request, respondWith } = evt;
      assert(!request.body);
      await respondWith(new Response(stream.readable));
      httpConn.close();
      listener.close();
    })();

    const resp = await fetch("http://127.0.0.1:4501/");
    const respBody = await resp.text();
    assertEquals("hello world", respBody);
    await promise;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerStreamRequest() {
    const stream = new TransformStream();
    const writer = stream.writable.getWriter();
    writer.write(new TextEncoder().encode("hello "));
    writer.write(new TextEncoder().encode("world"));
    writer.close();

    const promise = (async () => {
      const listener = Deno.listen({ port: 4501 });
      const conn = await listener.accept();
      const httpConn = Deno.serveHttp(conn);
      const evt = await httpConn.nextRequest();
      assert(evt);
      const { request, respondWith } = evt;
      const reqBody = await request.text();
      assertEquals("hello world", reqBody);
      await respondWith(new Response(""));

      // TODO(ry) If we don't call httpConn.nextRequest() here we get "error sending
      // request for url (https://localhost:4501/): connection closed before
      // message completed".
      assertEquals(await httpConn.nextRequest(), null);

      listener.close();
    })();

    const resp = await fetch("http://127.0.0.1:4501/", {
      body: stream.readable,
      method: "POST",
      headers: { "connection": "close" },
    });

    await resp.arrayBuffer();
    await promise;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerStreamDuplex() {
    const promise = (async () => {
      const listener = Deno.listen({ port: 4501 });
      const conn = await listener.accept();
      const httpConn = Deno.serveHttp(conn);
      const evt = await httpConn.nextRequest();
      assert(evt);
      const { request, respondWith } = evt;
      assert(request.body);
      await respondWith(new Response(request.body));
      httpConn.close();
      listener.close();
    })();

    const ts = new TransformStream();
    const writable = ts.writable.getWriter();
    const resp = await fetch("http://127.0.0.1:4501/", {
      method: "POST",
      body: ts.readable,
    });
    assert(resp.body);
    const reader = resp.body.getReader();
    await writable.write(new Uint8Array([1]));
    const chunk1 = await reader.read();
    assert(!chunk1.done);
    assertEquals(chunk1.value, new Uint8Array([1]));
    await writable.write(new Uint8Array([2]));
    const chunk2 = await reader.read();
    assert(!chunk2.done);
    assertEquals(chunk2.value, new Uint8Array([2]));
    await writable.close();
    const chunk3 = await reader.read();
    assert(chunk3.done);
    await promise;
  },
);

Deno.test({ permissions: { net: true } }, async function httpServerClose() {
  const listener = Deno.listen({ port: 4501 });
  const client = await Deno.connect({ port: 4501 });
  const httpConn = Deno.serveHttp(await listener.accept());
  client.close();
  const evt = await httpConn.nextRequest();
  assertEquals(evt, null);
  // Note httpConn is automatically closed when "done" is reached.
  listener.close();
});

Deno.test(
  { permissions: { net: true } },
  async function httpServerInvalidMethod() {
    const listener = Deno.listen({ port: 4501 });
    const client = await Deno.connect({ port: 4501 });
    const httpConn = Deno.serveHttp(await listener.accept());
    await client.write(new Uint8Array([1, 2, 3]));
    await assertRejects(
      async () => {
        await httpConn.nextRequest();
      },
      Deno.errors.Http,
      "invalid HTTP method parsed",
    );
    // Note httpConn is automatically closed when it errors.
    client.close();
    listener.close();
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function httpServerWithTls() {
    const hostname = "localhost";
    const port = 4501;

    const promise = (async () => {
      const listener = Deno.listenTls({
        hostname,
        port,
        certFile: "cli/tests/testdata/tls/localhost.crt",
        keyFile: "cli/tests/testdata/tls/localhost.key",
      });
      const conn = await listener.accept();
      const httpConn = Deno.serveHttp(conn);
      const evt = await httpConn.nextRequest();
      assert(evt);
      const { respondWith } = evt;
      await respondWith(new Response("Hello World"));

      // TODO(ry) If we don't call httpConn.nextRequest() here we get "error sending
      // request for url (https://localhost:4501/): connection closed before
      // message completed".
      assertEquals(await httpConn.nextRequest(), null);

      listener.close();
    })();

    const caCert = Deno.readTextFileSync("cli/tests/testdata/tls/RootCA.pem");
    const client = Deno.createHttpClient({ caCerts: [caCert] });
    const resp = await fetch(`https://${hostname}:${port}/`, {
      client,
      headers: { "connection": "close" },
    });
    const respBody = await resp.text();
    assertEquals("Hello World", respBody);
    await promise;
    client.close();
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerRegressionHang() {
    const promise = (async () => {
      const listener = Deno.listen({ port: 4501 });
      const conn = await listener.accept();
      const httpConn = Deno.serveHttp(conn);
      const event = await httpConn.nextRequest();
      assert(event);
      const { request, respondWith } = event;
      const reqBody = await request.text();
      assertEquals("request", reqBody);
      await respondWith(new Response("response"));
      httpConn.close();
      listener.close();
    })();

    const resp = await fetch("http://127.0.0.1:4501/", {
      method: "POST",
      body: "request",
    });
    const respBody = await resp.text();
    assertEquals("response", respBody);
    await promise;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerCancelBodyOnResponseFailure() {
    const promise = (async () => {
      const listener = Deno.listen({ port: 4501 });
      const conn = await listener.accept();
      const httpConn = Deno.serveHttp(conn);
      const event = await httpConn.nextRequest();
      assert(event);
      const { respondWith } = event;
      let cancelReason: string;
      await assertRejects(
        async () => {
          let interval = 0;
          await respondWith(
            new Response(
              new ReadableStream({
                start(controller) {
                  interval = setInterval(() => {
                    const message = `data: ${Date.now()}\n\n`;
                    controller.enqueue(new TextEncoder().encode(message));
                  }, 200);
                },
                cancel(reason) {
                  cancelReason = reason;
                  clearInterval(interval);
                },
              }),
            ),
          );
        },
        Deno.errors.Http,
        cancelReason!,
      );
      assert(cancelReason!);
      httpConn.close();
      listener.close();
    })();

    const resp = await fetch("http://127.0.0.1:4501/");
    await resp.body!.cancel();
    await promise;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerNextRequestErrorExposedInResponse() {
    const promise = (async () => {
      const listener = Deno.listen({ port: 4501 });
      const conn = await listener.accept();
      const httpConn = Deno.serveHttp(conn);
      const event = await httpConn.nextRequest();
      assert(event);
      // Start polling for the next request before awaiting response.
      const nextRequestPromise = httpConn.nextRequest();
      const { respondWith } = event;
      await assertRejects(
        async () => {
          let interval = 0;
          await respondWith(
            new Response(
              new ReadableStream({
                start(controller) {
                  interval = setInterval(() => {
                    const message = `data: ${Date.now()}\n\n`;
                    controller.enqueue(new TextEncoder().encode(message));
                  }, 200);
                },
                cancel() {
                  clearInterval(interval);
                },
              }),
            ),
          );
        },
        Deno.errors.Http,
        "connection closed",
      );
      // The error from `op_http_accept` reroutes to `respondWith()`.
      assertEquals(await nextRequestPromise, null);
      listener.close();
    })();

    const resp = await fetch("http://127.0.0.1:4501/");
    await resp.body!.cancel();
    await promise;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerEmptyBlobResponse() {
    const promise = (async () => {
      const listener = Deno.listen({ port: 4501 });
      const conn = await listener.accept();
      const httpConn = Deno.serveHttp(conn);
      const event = await httpConn.nextRequest();
      assert(event);
      const { respondWith } = event;
      await respondWith(new Response(new Blob([])));
      httpConn.close();
      listener.close();
    })();

    const resp = await fetch("http://127.0.0.1:4501/");
    const respBody = await resp.text();
    assertEquals("", respBody);
    await promise;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerNextRequestResolvesOnClose() {
    const httpConnList: Deno.HttpConn[] = [];

    async function serve(l: Deno.Listener) {
      for await (const conn of l) {
        (async () => {
          const c = Deno.serveHttp(conn);
          httpConnList.push(c);
          for await (const { respondWith } of c) {
            respondWith(new Response("hello"));
          }
        })();
      }
    }

    const l = Deno.listen({ port: 4500 });
    serve(l);

    await delay(300);
    const res = await fetch("http://localhost:4500/");
    const _text = await res.text();

    // Close connection and listener.
    httpConnList.forEach((conn) => conn.close());
    l.close();

    await delay(300);
  },
);

Deno.test(
  { permissions: { net: true } },
  // Issue: https://github.com/denoland/deno/issues/10870
  async function httpServerHang() {
    // Quick and dirty way to make a readable stream from a string. Alternatively,
    // `readableStreamFromReader(file)` could be used.
    function stream(s: string): ReadableStream<Uint8Array> {
      return new Response(s).body!;
    }

    const httpConns: Deno.HttpConn[] = [];
    const promise = (async () => {
      let count = 0;
      const listener = Deno.listen({ port: 4501 });
      for await (const conn of listener) {
        (async () => {
          const httpConn = Deno.serveHttp(conn);
          httpConns.push(httpConn);
          for await (const { respondWith } of httpConn) {
            respondWith(new Response(stream("hello")));

            count++;
            if (count >= 2) {
              listener.close();
            }
          }
        })();
      }
    })();

    const clientConn = await Deno.connect({ port: 4501 });

    const r1 = await writeRequestAndReadResponse(clientConn);
    assertEquals(r1, "hello");

    const r2 = await writeRequestAndReadResponse(clientConn);
    assertEquals(r2, "hello");

    clientConn.close();
    await promise;
    for (const conn of httpConns) {
      conn.close();
    }
  },
);

Deno.test(
  { permissions: { net: true } },
  // Issue: https://github.com/denoland/deno/issues/10930
  async function httpServerStreamingResponse() {
    // This test enqueues a single chunk for readable
    // stream and waits for client to read that chunk and signal
    // it before enqueueing subsequent chunk. Issue linked above
    // presented a situation where enqueued chunks were not
    // written to the HTTP connection until the next chunk was enqueued.

    let counter = 0;

    const deferreds = [
      deferred(),
      deferred(),
      deferred(),
    ];

    async function writeRequest(conn: Deno.Conn) {
      const encoder = new TextEncoder();
      const decoder = new TextDecoder();

      const w = new BufWriter(conn);
      const r = new BufReader(conn);
      const body = `GET / HTTP/1.1\r\nHost: 127.0.0.1:4501\r\n\r\n`;
      const writeResult = await w.write(encoder.encode(body));
      assertEquals(body.length, writeResult);
      await w.flush();
      const tpr = new TextProtoReader(r);
      const statusLine = await tpr.readLine();
      assert(statusLine !== null);
      const headers = await tpr.readMIMEHeader();
      assert(headers !== null);

      const chunkedReader = chunkedBodyReader(headers, r);
      const buf = new Uint8Array(5);
      const dest = new Buffer();
      let result: number | null;
      while ((result = await chunkedReader.read(buf)) !== null) {
        const len = Math.min(buf.byteLength, result);
        await dest.write(buf.subarray(0, len));
        // Resolve a deferred - this will make response stream to
        // enqueue next chunk.
        deferreds[counter - 1].resolve();
      }
      return decoder.decode(dest.bytes());
    }

    function periodicStream() {
      return new ReadableStream({
        start(controller) {
          controller.enqueue(`${counter}\n`);
          counter++;
        },

        async pull(controller) {
          if (counter >= 3) {
            return controller.close();
          }

          await deferreds[counter - 1];

          controller.enqueue(`${counter}\n`);
          counter++;
        },
      }).pipeThrough(new TextEncoderStream());
    }

    const listener = Deno.listen({ port: 4501 });
    const finished = (async () => {
      const conn = await listener.accept();
      const httpConn = Deno.serveHttp(conn);
      const requestEvent = await httpConn.nextRequest();
      const { respondWith } = requestEvent!;
      await respondWith(new Response(periodicStream()));
      httpConn.close();
    })();

    // start a client
    const clientConn = await Deno.connect({ port: 4501 });

    const r1 = await writeRequest(clientConn);
    assertEquals(r1, "0\n1\n2\n");

    await finished;
    clientConn.close();
    listener.close();
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpRequestLatin1Headers() {
    const promise = (async () => {
      const listener = Deno.listen({ port: 4501 });
      for await (const conn of listener) {
        const httpConn = Deno.serveHttp(conn);
        for await (const { request, respondWith } of httpConn) {
          assertEquals(request.headers.get("X-Header-Test"), "á");
          await respondWith(
            new Response("", { headers: { "X-Header-Test": "Æ" } }),
          );
          httpConn.close();
        }
        break;
      }
    })();

    const clientConn = await Deno.connect({ port: 4501 });
    const requestText =
      "GET / HTTP/1.1\r\nHost: 127.0.0.1:4501\r\nX-Header-Test: á\r\n\r\n";
    const requestBytes = new Uint8Array(requestText.length);
    for (let i = 0; i < requestText.length; i++) {
      requestBytes[i] = requestText.charCodeAt(i);
    }
    let written = 0;
    while (written < requestBytes.byteLength) {
      written += await clientConn.write(requestBytes.slice(written));
    }

    let responseText = "";
    const buf = new Uint8Array(1024);
    let read;
    while ((read = await clientConn.read(buf)) !== null) {
      for (let i = 0; i < read; i++) {
        responseText += String.fromCharCode(buf[i]);
      }
    }
    clientConn.close();

    assert(/\r\n[Xx]-[Hh]eader-[Tt]est: Æ\r\n/.test(responseText));

    await promise;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerRequestWithoutPath() {
    const promise = (async () => {
      const listener = Deno.listen({ port: 4501 });
      for await (const conn of listener) {
        const httpConn = Deno.serveHttp(conn);
        for await (const { request, respondWith } of httpConn) {
          assertEquals(new URL(request.url).href, "http://127.0.0.1:4501/");
          assertEquals(await request.text(), "");
          respondWith(new Response());
        }
        break;
      }
    })();

    const clientConn = await Deno.connect({ port: 4501 });

    async function writeRequest(conn: Deno.Conn) {
      const encoder = new TextEncoder();

      const w = new BufWriter(conn);
      const r = new BufReader(conn);
      const body =
        `CONNECT 127.0.0.1:4501 HTTP/1.1\r\nHost: 127.0.0.1:4501\r\n\r\n`;
      const writeResult = await w.write(encoder.encode(body));
      assertEquals(body.length, writeResult);
      await w.flush();
      const tpr = new TextProtoReader(r);
      const statusLine = await tpr.readLine();
      assert(statusLine !== null);
      const m = statusLine.match(/^(.+?) (.+?) (.+?)$/);
      assert(m !== null, "must be matched");
      const [_, _proto, status, _ok] = m;
      assertEquals(status, "200");
      const headers = await tpr.readMIMEHeader();
      assert(headers !== null);
    }

    await writeRequest(clientConn);
    clientConn.close();
    await promise;
  },
);

Deno.test({ permissions: { net: true } }, async function httpServerWebSocket() {
  const promise = (async () => {
    const listener = Deno.listen({ port: 4501 });
    for await (const conn of listener) {
      const httpConn = Deno.serveHttp(conn);
      const { request, respondWith } = (await httpConn.nextRequest())!;
      const {
        response,
        socket,
      } = Deno.upgradeWebSocket(request);
      socket.onerror = () => fail();
      socket.onmessage = (m) => {
        socket.send(m.data);
        socket.close(1001);
      };
      await respondWith(response);
      break;
    }
  })();

  const def = deferred();
  const ws = new WebSocket("ws://localhost:4501");
  ws.onmessage = (m) => assertEquals(m.data, "foo");
  ws.onerror = () => fail();
  ws.onclose = () => def.resolve();
  ws.onopen = () => ws.send("foo");
  await def;
  await promise;
});

Deno.test(function httpUpgradeWebSocket() {
  const request = new Request("https://deno.land/", {
    headers: {
      connection: "Upgrade",
      upgrade: "websocket",
      "sec-websocket-key": "dGhlIHNhbXBsZSBub25jZQ==",
    },
  });
  const { response } = Deno.upgradeWebSocket(request);
  assertEquals(response.status, 101);
  assertEquals(response.headers.get("connection"), "Upgrade");
  assertEquals(response.headers.get("upgrade"), "websocket");
  assertEquals(
    response.headers.get("sec-websocket-accept"),
    "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=",
  );
});

Deno.test(function httpUpgradeWebSocketMultipleConnectionOptions() {
  const request = new Request("https://deno.land/", {
    headers: {
      connection: "keep-alive, upgrade",
      upgrade: "websocket",
      "sec-websocket-key": "dGhlIHNhbXBsZSBub25jZQ==",
    },
  });
  const { response } = Deno.upgradeWebSocket(request);
  assertEquals(response.status, 101);
});

Deno.test(function httpUpgradeWebSocketMultipleUpgradeOptions() {
  const request = new Request("https://deno.land/", {
    headers: {
      connection: "upgrade",
      upgrade: "websocket, foo",
      "sec-websocket-key": "dGhlIHNhbXBsZSBub25jZQ==",
    },
  });
  const { response } = Deno.upgradeWebSocket(request);
  assertEquals(response.status, 101);
});

Deno.test(function httpUpgradeWebSocketCaseInsensitiveUpgradeHeader() {
  const request = new Request("https://deno.land/", {
    headers: {
      connection: "upgrade",
      upgrade: "Websocket",
      "sec-websocket-key": "dGhlIHNhbXBsZSBub25jZQ==",
    },
  });
  const { response } = Deno.upgradeWebSocket(request);
  assertEquals(response.status, 101);
});

Deno.test(function httpUpgradeWebSocketInvalidUpgradeHeader() {
  assertThrows(
    () => {
      const request = new Request("https://deno.land/", {
        headers: {
          connection: "upgrade",
          upgrade: "invalid",
          "sec-websocket-key": "dGhlIHNhbXBsZSBub25jZQ==",
        },
      });
      Deno.upgradeWebSocket(request);
    },
    TypeError,
    "Invalid Header: 'upgrade' header must contain 'websocket'",
  );
});

Deno.test(function httpUpgradeWebSocketWithoutUpgradeHeader() {
  assertThrows(
    () => {
      const request = new Request("https://deno.land/", {
        headers: {
          connection: "upgrade",
          "sec-websocket-key": "dGhlIHNhbXBsZSBub25jZQ==",
        },
      });
      Deno.upgradeWebSocket(request);
    },
    TypeError,
    "Invalid Header: 'upgrade' header must contain 'websocket'",
  );
});

Deno.test(
  { permissions: { net: true } },
  async function httpCookieConcatenation() {
    const promise = (async () => {
      const listener = Deno.listen({ port: 4501 });
      for await (const conn of listener) {
        const httpConn = Deno.serveHttp(conn);
        for await (const { request, respondWith } of httpConn) {
          assertEquals(new URL(request.url).href, "http://127.0.0.1:4501/");
          assertEquals(await request.text(), "");
          assertEquals(request.headers.get("cookie"), "foo=bar; bar=foo");
          respondWith(new Response("ok"));
        }
        break;
      }
    })();

    const resp = await fetch("http://127.0.0.1:4501/", {
      headers: [
        ["connection", "close"],
        ["cookie", "foo=bar"],
        ["cookie", "bar=foo"],
      ],
    });
    const text = await resp.text();
    assertEquals(text, "ok");
    await promise;
  },
);

// https://github.com/denoland/deno/issues/11651
Deno.test({ permissions: { net: true } }, async function httpServerPanic() {
  const listener = Deno.listen({ port: 4501 });
  const client = await Deno.connect({ port: 4501 });
  const conn = await listener.accept();
  const httpConn = Deno.serveHttp(conn);

  // This message is incomplete on purpose, we'll forcefully close client connection
  // after it's flushed to cause connection to error out on the server side.
  const encoder = new TextEncoder();
  await client.write(encoder.encode("GET / HTTP/1.1"));

  httpConn.nextRequest();
  await client.write(encoder.encode("\r\n\r\n"));
  httpConn.close();

  client.close();
  listener.close();
});

// https://github.com/denoland/deno/issues/11595
Deno.test(
  { permissions: { net: true } },
  async function httpServerIncompleteMessage() {
    const listener = Deno.listen({ port: 4501 });

    const client = await Deno.connect({ port: 4501 });
    await client.write(new TextEncoder().encode(
      `GET / HTTP/1.0\r\n\r\n`,
    ));

    const conn = await listener.accept();
    const httpConn = Deno.serveHttp(conn);
    const ev = await httpConn.nextRequest();
    const { respondWith } = ev!;

    const errors: Error[] = [];

    const readable = new ReadableStream({
      async pull(controller) {
        client.close();
        await delay(1000);
        controller.enqueue(new TextEncoder().encode(
          "written to the writable side of a TransformStream",
        ));
        controller.close();
      },
      cancel(error) {
        errors.push(error);
      },
    });

    const res = new Response(readable);

    await respondWith(res).catch((error: Error) => errors.push(error));

    httpConn.close();
    listener.close();

    assert(errors.length >= 1);
    for (const error of errors) {
      assertEquals(error.name, "Http");
      assert(error.message.includes("connection"));
    }
  },
);

// https://github.com/denoland/deno/issues/11743
Deno.test(
  { permissions: { net: true } },
  async function httpServerDoesntLeakResources() {
    const listener = Deno.listen({ port: 4505 });
    const [conn, clientConn] = await Promise.all([
      listener.accept(),
      Deno.connect({ port: 4505 }),
    ]);
    const httpConn = Deno.serveHttp(conn);

    await Promise.all([
      httpConn.nextRequest(),
      clientConn.write(new TextEncoder().encode(
        `GET / HTTP/1.1\r\nHost: 127.0.0.1:4505\r\n\r\n`,
      )),
    ]);

    httpConn.close();
    listener.close();
    clientConn.close();
  },
);

// https://github.com/denoland/deno/issues/11926
Deno.test(
  { permissions: { net: true } },
  async function httpServerDoesntLeakResources2() {
    let listener: Deno.Listener;
    let httpConn: Deno.HttpConn;

    const promise = (async () => {
      listener = Deno.listen({ port: 4508 });
      for await (const conn of listener) {
        httpConn = Deno.serveHttp(conn);
        for await (const { request, respondWith } of httpConn) {
          assertEquals(new URL(request.url).href, "http://127.0.0.1:4508/");
          // not reading request body on purpose
          respondWith(new Response("ok"));
        }
      }
    })();

    const resourcesBefore = Deno.resources();
    const response = await fetch("http://127.0.0.1:4508", {
      method: "POST",
      body: "hello world",
    });
    await response.text();
    const resourcesAfter = Deno.resources();
    // verify that the only new resource is "httpConnection", to make
    // sure "request" resource is closed even if its body was not read
    // by server handler

    for (const rid of Object.keys(resourcesBefore)) {
      delete resourcesAfter[Number(rid)];
    }

    assertEquals(Object.keys(resourcesAfter).length, 1);

    listener!.close();
    httpConn!.close();
    await promise;
  },
);

// https://github.com/denoland/deno/pull/12216
Deno.test(
  { permissions: { net: true } },
  async function droppedConnSenderNoPanic() {
    async function server() {
      const listener = Deno.listen({ port: 8000 });
      const conn = await listener.accept();
      const http = Deno.serveHttp(conn);
      const evt = await http.nextRequest();
      http.close();
      try {
        await evt!.respondWith(new Response("boom"));
      } catch {
        // Ignore error.
      }
      listener.close();
    }

    async function client() {
      try {
        const resp = await fetch("http://127.0.0.1:8000/");
        await resp.body?.cancel();
      } catch {
        // Ignore error
      }
    }

    await Promise.all([server(), client()]);
  },
);

// https://github.com/denoland/deno/issues/12193
Deno.test(
  { permissions: { net: true } },
  async function httpConnConcurrentNextRequestCalls() {
    const hostname = "localhost";
    const port = 4501;

    async function server() {
      const listener = Deno.listen({ hostname, port });
      const tcpConn = await listener.accept();
      const httpConn = Deno.serveHttp(tcpConn);
      const promises = new Array(10).fill(null).map(async (_, i) => {
        const event = await httpConn.nextRequest();
        assert(event);
        const { pathname } = new URL(event.request.url);
        assertStrictEquals(pathname, `/${i}`);
        const response = new Response(`Response #${i}`);
        await event.respondWith(response);
      });
      await Promise.all(promises);
      httpConn.close();
      listener.close();
    }

    async function client() {
      for (let i = 0; i < 10; i++) {
        const response = await fetch(`http://${hostname}:${port}/${i}`);
        const body = await response.text();
        assertStrictEquals(body, `Response #${i}`);
      }
    }

    await Promise.all([server(), delay(100).then(client)]);
  },
);

// https://github.com/denoland/deno/pull/12704
// https://github.com/denoland/deno/pull/12732
Deno.test(
  { permissions: { net: true } },
  async function httpConnAutoCloseDelayedOnUpgrade() {
    const hostname = "localhost";
    const port = 4501;

    async function server() {
      const listener = Deno.listen({ hostname, port });
      const tcpConn = await listener.accept();
      const httpConn = Deno.serveHttp(tcpConn);

      const event1 = await httpConn.nextRequest() as Deno.RequestEvent;
      const event2Promise = httpConn.nextRequest();

      const { socket, response } = Deno.upgradeWebSocket(event1.request);
      socket.onmessage = (event) => socket.send(event.data);
      const socketClosed = new Promise<void>((resolve) => {
        socket.onclose = () => resolve();
      });
      event1.respondWith(response);

      const event2 = await event2Promise;
      assertStrictEquals(event2, null);

      listener.close();
      await socketClosed;
    }

    async function client() {
      const socket = new WebSocket(`ws://${hostname}:${port}/`);
      socket.onopen = () => socket.send("bla bla");
      const { data } = await new Promise((res) => socket.onmessage = res);
      assertStrictEquals(data, "bla bla");
      socket.close();
    }

    await Promise.all([server(), client()]);
  },
);

// https://github.com/denoland/deno/issues/12741
// https://github.com/denoland/deno/pull/12746
// https://github.com/denoland/deno/pull/12798
Deno.test(
  { permissions: { net: true, run: true } },
  async function httpServerDeleteRequestHasBody() {
    const hostname = "localhost";
    const port = 4501;

    async function server() {
      const listener = Deno.listen({ hostname, port });
      const tcpConn = await listener.accept();
      const httpConn = Deno.serveHttp(tcpConn);
      const event = await httpConn.nextRequest() as Deno.RequestEvent;
      assert(event.request.body);
      const response = new Response();
      await event.respondWith(response);
      httpConn.close();
      listener.close();
    }

    async function client() {
      const url = `http://${hostname}:${port}/`;
      const cmd = ["curl", "-X", "DELETE", url];
      const proc = Deno.run({ cmd, stdout: "null", stderr: "null" });
      const status = await proc.status();
      assert(status.success);
      proc.close();
    }

    await Promise.all([server(), client()]);
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerRespondNonAsciiUint8Array() {
    const promise = (async () => {
      const listener = Deno.listen({ port: 4501 });
      const conn = await listener.accept();
      listener.close();
      const httpConn = Deno.serveHttp(conn);
      const e = await httpConn.nextRequest();
      assert(e);
      const { request, respondWith } = e;
      assertEquals(request.body, null);
      await respondWith(
        new Response(new Uint8Array([128]), {}),
      );
      httpConn.close();
    })();

    const resp = await fetch("http://localhost:4501/");
    assertEquals(resp.status, 200);
    const body = await resp.arrayBuffer();
    assertEquals(new Uint8Array(body), new Uint8Array([128]));

    await promise;
  },
);

// https://github.com/denoland/deno/pull/13628
Deno.test(
  {
    ignore: Deno.build.os === "windows",
    permissions: { read: true, write: true },
  },
  async function httpServerOnUnixSocket() {
    const filePath = Deno.makeTempFileSync();

    const promise = (async () => {
      const listener = Deno.listen({ path: filePath, transport: "unix" });
      for await (const conn of listener) {
        const httpConn = Deno.serveHttp(conn);
        for await (const { request, respondWith } of httpConn) {
          const url = new URL(request.url);
          assertEquals(url.protocol, "http+unix:");
          assertEquals(decodeURIComponent(url.host), filePath);
          assertEquals(url.pathname, "/path/name");
          await respondWith(new Response("", { headers: {} }));
          httpConn.close();
        }
        break;
      }
    })();

    // fetch() does not supports unix domain sockets yet https://github.com/denoland/deno/issues/8821
    const conn = await Deno.connect({ path: filePath, transport: "unix" });
    const encoder = new TextEncoder();
    // The Host header must be present and empty if it is not a Internet host name (RFC2616, Section 14.23)
    const body = `GET /path/name HTTP/1.1\r\nHost:\r\n\r\n`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);

    const resp = new Uint8Array(200);
    const readResult = await conn.read(resp);
    assertEquals(readResult, 115);

    conn.close();

    await promise;
  },
);

function chunkedBodyReader(h: Headers, r: BufReader): Deno.Reader {
  // Based on https://tools.ietf.org/html/rfc2616#section-19.4.6
  const tp = new TextProtoReader(r);
  let finished = false;
  const chunks: Array<{
    offset: number;
    data: Uint8Array;
  }> = [];
  async function read(buf: Uint8Array): Promise<number | null> {
    if (finished) return null;
    const [chunk] = chunks;
    if (chunk) {
      const chunkRemaining = chunk.data.byteLength - chunk.offset;
      const readLength = Math.min(chunkRemaining, buf.byteLength);
      for (let i = 0; i < readLength; i++) {
        buf[i] = chunk.data[chunk.offset + i];
      }
      chunk.offset += readLength;
      if (chunk.offset === chunk.data.byteLength) {
        chunks.shift();
        // Consume \r\n;
        if ((await tp.readLine()) === null) {
          throw new Deno.errors.UnexpectedEof();
        }
      }
      return readLength;
    }
    const line = await tp.readLine();
    if (line === null) throw new Deno.errors.UnexpectedEof();
    // TODO(bartlomieju): handle chunk extension
    const [chunkSizeString] = line.split(";");
    const chunkSize = parseInt(chunkSizeString, 16);
    if (Number.isNaN(chunkSize) || chunkSize < 0) {
      throw new Deno.errors.InvalidData("Invalid chunk size");
    }
    if (chunkSize > 0) {
      if (chunkSize > buf.byteLength) {
        let eof = await r.readFull(buf);
        if (eof === null) {
          throw new Deno.errors.UnexpectedEof();
        }
        const restChunk = new Uint8Array(chunkSize - buf.byteLength);
        eof = await r.readFull(restChunk);
        if (eof === null) {
          throw new Deno.errors.UnexpectedEof();
        } else {
          chunks.push({
            offset: 0,
            data: restChunk,
          });
        }
        return buf.byteLength;
      } else {
        const bufToFill = buf.subarray(0, chunkSize);
        const eof = await r.readFull(bufToFill);
        if (eof === null) {
          throw new Deno.errors.UnexpectedEof();
        }
        // Consume \r\n
        if ((await tp.readLine()) === null) {
          throw new Deno.errors.UnexpectedEof();
        }
        return chunkSize;
      }
    } else {
      assert(chunkSize === 0);
      // Consume \r\n
      if ((await r.readLine()) === null) {
        throw new Deno.errors.UnexpectedEof();
      }
      await readTrailers(h, r);
      finished = true;
      return null;
    }
  }
  return { read };
}

async function readTrailers(
  headers: Headers,
  r: BufReader,
) {
  const trailers = parseTrailer(headers.get("trailer"));
  if (trailers == null) return;
  const trailerNames = [...trailers.keys()];
  const tp = new TextProtoReader(r);
  const result = await tp.readMIMEHeader();
  if (result == null) {
    throw new Deno.errors.InvalidData("Missing trailer header.");
  }
  const undeclared = [...result.keys()].filter(
    (k) => !trailerNames.includes(k),
  );
  if (undeclared.length > 0) {
    throw new Deno.errors.InvalidData(
      `Undeclared trailers: ${Deno.inspect(undeclared)}.`,
    );
  }
  for (const [k, v] of result) {
    headers.append(k, v);
  }
  const missingTrailers = trailerNames.filter((k) => !result.has(k));
  if (missingTrailers.length > 0) {
    throw new Deno.errors.InvalidData(
      `Missing trailers: ${Deno.inspect(missingTrailers)}.`,
    );
  }
  headers.delete("trailer");
}

function parseTrailer(field: string | null): Headers | undefined {
  if (field == null) {
    return undefined;
  }
  const trailerNames = field.split(",").map((v) => v.trim().toLowerCase());
  if (trailerNames.length === 0) {
    throw new Deno.errors.InvalidData("Empty trailer header.");
  }
  const prohibited = trailerNames.filter((k) => isProhibitedForTrailer(k));
  if (prohibited.length > 0) {
    throw new Deno.errors.InvalidData(
      `Prohibited trailer names: ${Deno.inspect(prohibited)}.`,
    );
  }
  return new Headers(trailerNames.map((key) => [key, ""]));
}

function isProhibitedForTrailer(key: string): boolean {
  const s = new Set(["transfer-encoding", "content-length", "trailer"]);
  return s.has(key.toLowerCase());
}
