// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { chunkedBodyReader } from "../../../test_util/std/http/_io.ts";
import { BufReader, BufWriter } from "../../../test_util/std/io/bufio.ts";
import { Buffer } from "../../../test_util/std/io/buffer.ts";
import { TextProtoReader } from "../../../test_util/std/textproto/mod.ts";
import {
  assert,
  assertEquals,
  assertThrowsAsync,
  deferred,
  delay,
  fail,
  unitTest,
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

unitTest({ perms: { net: true } }, async function httpServerBasic() {
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

unitTest(
  { perms: { net: true } },
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

unitTest(
  { perms: { net: true } },
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

unitTest({ perms: { net: true } }, async function httpServerStreamDuplex() {
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
});

unitTest({ perms: { net: true } }, async function httpServerClose() {
  const listener = Deno.listen({ port: 4501 });
  const client = await Deno.connect({ port: 4501 });
  const httpConn = Deno.serveHttp(await listener.accept());
  client.close();
  const evt = await httpConn.nextRequest();
  assertEquals(evt, null);
  // Note httpConn is automatically closed when "done" is reached.
  listener.close();
});

unitTest({ perms: { net: true } }, async function httpServerInvalidMethod() {
  const listener = Deno.listen({ port: 4501 });
  const client = await Deno.connect({ port: 4501 });
  const httpConn = Deno.serveHttp(await listener.accept());
  await client.write(new Uint8Array([1, 2, 3]));
  await assertThrowsAsync(
    async () => {
      await httpConn.nextRequest();
    },
    Deno.errors.Http,
    "invalid HTTP method parsed",
  );
  // Note httpConn is automatically closed when it errors.
  client.close();
  listener.close();
});

unitTest(
  { perms: { read: true, net: true } },
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

    const caData = Deno.readTextFileSync("cli/tests/testdata/tls/RootCA.pem");
    const client = Deno.createHttpClient({ caData });
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

unitTest(
  { perms: { net: true } },
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

unitTest(
  { perms: { net: true } },
  async function httpServerCancelBodyOnResponseFailure() {
    const promise = (async () => {
      const listener = Deno.listen({ port: 4501 });
      const conn = await listener.accept();
      const httpConn = Deno.serveHttp(conn);
      const event = await httpConn.nextRequest();
      assert(event);
      const { respondWith } = event;
      let cancelReason = null;
      const responseError = await assertThrowsAsync(
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
      );
      assertEquals(cancelReason, responseError);
      httpConn.close();
      listener.close();
    })();

    const resp = await fetch("http://127.0.0.1:4501/");
    await resp.body!.cancel();
    await promise;
  },
);

unitTest(
  { perms: { net: true } },
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
      await assertThrowsAsync(
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
      // The error from `op_http_request_next` reroutes to `respondWith()`.
      assertEquals(await nextRequestPromise, null);
      listener.close();
    })();

    const resp = await fetch("http://127.0.0.1:4501/");
    await resp.body!.cancel();
    await promise;
  },
);

unitTest(
  { perms: { net: true } },
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

unitTest(
  { perms: { net: true } },
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

unitTest(
  { perms: { net: true } },
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

unitTest(
  { perms: { net: true } },
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

unitTest({ perms: { net: true } }, async function httpRequestLatin1Headers() {
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
});

unitTest(
  { perms: { net: true } },
  async function httpServerRequestWithoutPath() {
    const promise = (async () => {
      const listener = Deno.listen({ port: 4501 });
      for await (const conn of listener) {
        const httpConn = Deno.serveHttp(conn);
        for await (const { request, respondWith } of httpConn) {
          assertEquals(new URL(request.url).href, "http://127.0.0.1/");
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

unitTest({ perms: { net: true } }, async function httpServerWebSocket() {
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

unitTest(function httpUpgradeWebSocket() {
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

unitTest(function httpUpgradeWebSocketLowercaseUpgradeHeader() {
  const request = new Request("https://deno.land/", {
    headers: {
      connection: "upgrade",
      upgrade: "websocket",
      "sec-websocket-key": "dGhlIHNhbXBsZSBub25jZQ==",
    },
  });
  const { response } = Deno.upgradeWebSocket(request);
  assertEquals(response.status, 101);
});

unitTest(function httpUpgradeWebSocketMultipleConnectionOptions() {
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

unitTest({ perms: { net: true } }, async function httpCookieConcatenation() {
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
});

// https://github.com/denoland/deno/issues/11651
unitTest({ perms: { net: true } }, async function httpServerPanic() {
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
unitTest(
  { perms: { net: true } },
  async function httpServerIncompleteMessage() {
    const listener = Deno.listen({ port: 4501 });
    const def1 = deferred();
    const def2 = deferred();

    const client = await Deno.connect({ port: 4501 });
    await client.write(new TextEncoder().encode(
      `GET / HTTP/1.0\r\n\r\n`,
    ));

    const conn = await listener.accept();
    const httpConn = Deno.serveHttp(conn);
    const ev = await httpConn.nextRequest();
    const { respondWith } = ev!;

    const { readable, writable } = new TransformStream<Uint8Array>();
    const writer = writable.getWriter();

    async function writeResponse() {
      await writer.write(
        new TextEncoder().encode(
          "written to the writable side of a TransformStream",
        ),
      );
      await writer.close();
    }

    const errors: Error[] = [];

    writeResponse()
      .catch((error: Error) => {
        errors.push(error);
      })
      .then(() => def1.resolve());

    const res = new Response(readable);

    respondWith(res)
      .catch((error: Error) => errors.push(error))
      .then(() => def2.resolve());

    client.close();

    await Promise.all([
      def1,
      def2,
    ]);

    listener.close();

    assertEquals(errors.length, 2);
    for (const error of errors) {
      assertEquals(error.name, "Http");
      assertEquals(
        error.message,
        "connection closed before message completed",
      );
    }
  },
);

// https://github.com/denoland/deno/issues/11743
unitTest(
  { perms: { net: true } },
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
