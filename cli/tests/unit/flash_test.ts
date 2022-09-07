// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file

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
  assertThrows,
  Deferred,
  deferred,
  fail,
} from "./test_util.ts";

function createOnErrorCb(ac: AbortController): (err: unknown) => Response {
  return (err) => {
    console.error(err);
    ac.abort();
    return new Response("Internal server error", { status: 500 });
  };
}

function onListen<T>(
  p: Deferred<T>,
): ({ hostname, port }: { hostname: string; port: number }) => void {
  return () => {
    p.resolve();
  };
}

Deno.test(async function httpServerCanResolveHostnames() {
  const ac = new AbortController();
  const listeningPromise = deferred();

  const server = Deno.serve({
    handler: (_req) => new Response("ok"),
    hostname: "localhost",
    port: 4501,
    signal: ac.signal,
    onListen: onListen(listeningPromise),
    onError: createOnErrorCb(ac),
  });

  await listeningPromise;
  const resp = await fetch("http://localhost:4501/", {
    headers: { "connection": "close" },
  });
  const text = await resp.text();
  assertEquals(text, "ok");
  ac.abort();
  await server;
});

Deno.test(async function httpServerRejectsOnAddrInUse() {
  const ac = new AbortController();
  const listeningPromise = deferred();

  const server = Deno.serve({
    handler: (_req) => new Response("ok"),
    hostname: "localhost",
    port: 4501,
    signal: ac.signal,
    onListen: onListen(listeningPromise),
    onError: createOnErrorCb(ac),
  });

  assertRejects(
    () =>
      Deno.serve({
        handler: (_req) => new Response("ok"),
        hostname: "localhost",
        port: 4501,
        signal: ac.signal,
        onListen: onListen(listeningPromise),
        onError: createOnErrorCb(ac),
      }),
    Deno.errors.AddrInUse,
  );
  ac.abort();
  await server;
});

Deno.test({ permissions: { net: true } }, async function httpServerBasic() {
  const ac = new AbortController();
  const promise = deferred();
  const listeningPromise = deferred();

  const server = Deno.serve({
    handler: async (request) => {
      // FIXME(bartlomieju):
      // make sure that request can be inspected
      console.log(request);
      assertEquals(new URL(request.url).href, "http://127.0.0.1:4501/");
      assertEquals(await request.text(), "");
      promise.resolve();
      return new Response("Hello World", { headers: { "foo": "bar" } });
    },
    port: 4501,
    signal: ac.signal,
    onListen: onListen(listeningPromise),
    onError: createOnErrorCb(ac),
  });

  await listeningPromise;
  const resp = await fetch("http://127.0.0.1:4501/", {
    headers: { "connection": "close" },
  });
  await promise;
  const clone = resp.clone();
  const text = await resp.text();
  assertEquals(text, "Hello World");
  assertEquals(resp.headers.get("foo"), "bar");
  const cloneText = await clone.text();
  assertEquals(cloneText, "Hello World");
  ac.abort();
  await server;
});

Deno.test({ permissions: { net: true } }, async function httpServerOverload1() {
  const ac = new AbortController();
  const promise = deferred();
  const listeningPromise = deferred();

  const server = Deno.serve({
    port: 4501,
    signal: ac.signal,
    onListen: onListen(listeningPromise),
    onError: createOnErrorCb(ac),
  }, async (request) => {
    // FIXME(bartlomieju):
    // make sure that request can be inspected
    console.log(request);
    assertEquals(new URL(request.url).href, "http://127.0.0.1:4501/");
    assertEquals(await request.text(), "");
    promise.resolve();
    return new Response("Hello World", { headers: { "foo": "bar" } });
  });

  await listeningPromise;
  const resp = await fetch("http://127.0.0.1:4501/", {
    headers: { "connection": "close" },
  });
  await promise;
  const clone = resp.clone();
  const text = await resp.text();
  assertEquals(text, "Hello World");
  assertEquals(resp.headers.get("foo"), "bar");
  const cloneText = await clone.text();
  assertEquals(cloneText, "Hello World");
  ac.abort();
  await server;
});

Deno.test({ permissions: { net: true } }, async function httpServerOverload2() {
  const ac = new AbortController();
  const promise = deferred();
  const listeningPromise = deferred();

  const server = Deno.serve(async (request) => {
    // FIXME(bartlomieju):
    // make sure that request can be inspected
    console.log(request);
    assertEquals(new URL(request.url).href, "http://127.0.0.1:4501/");
    assertEquals(await request.text(), "");
    promise.resolve();
    return new Response("Hello World", { headers: { "foo": "bar" } });
  }, {
    port: 4501,
    signal: ac.signal,
    onListen: onListen(listeningPromise),
    onError: createOnErrorCb(ac),
  });

  await listeningPromise;
  const resp = await fetch("http://127.0.0.1:4501/", {
    headers: { "connection": "close" },
  });
  await promise;
  const clone = resp.clone();
  const text = await resp.text();
  assertEquals(text, "Hello World");
  assertEquals(resp.headers.get("foo"), "bar");
  const cloneText = await clone.text();
  assertEquals(cloneText, "Hello World");
  ac.abort();
  await server;
});

Deno.test(
  { permissions: { net: true } },
  async function httpServerErrorOverloadMissingHandler() {
    // @ts-ignore - testing invalid overload
    await assertRejects(() => Deno.serve(), TypeError, "handler");
    // @ts-ignore - testing invalid overload
    await assertRejects(() => Deno.serve({}), TypeError, "handler");
    await assertRejects(
      // @ts-ignore - testing invalid overload
      () => Deno.serve({ handler: undefined }),
      TypeError,
      "handler",
    );
    await assertRejects(
      // @ts-ignore - testing invalid overload
      () => Deno.serve(undefined, { handler: () => {} }),
      TypeError,
      "handler",
    );
  },
);

Deno.test({ permissions: { net: true } }, async function httpServerPort0() {
  const ac = new AbortController();

  const server = Deno.serve({
    handler() {
      return new Response("Hello World");
    },
    port: 0,
    signal: ac.signal,
    onListen({ port }) {
      assert(port > 0 && port < 65536);
      ac.abort();
    },
  });
  await server;
});

Deno.test(
  { permissions: { net: true } },
  async function httpServerDefaultOnListenCallback() {
    const ac = new AbortController();

    const consoleLog = console.log;
    console.log = (msg) => {
      try {
        const match = msg.match(/Listening on http:\/\/localhost:(\d+)\//);
        assert(!!match);
        const port = +match[1];
        assert(port > 0 && port < 65536);
      } finally {
        ac.abort();
      }
    };

    try {
      const server = Deno.serve({
        handler() {
          return new Response("Hello World");
        },
        hostname: "0.0.0.0",
        port: 0,
        signal: ac.signal,
      });

      await server;
    } finally {
      console.log = consoleLog;
    }
  },
);

// https://github.com/denoland/deno/issues/15107
Deno.test(
  { permissions: { net: true } },
  async function httpLazyHeadersIssue15107() {
    const promise = deferred();
    const listeningPromise = deferred();
    const ac = new AbortController();

    let headers: Headers;
    const server = Deno.serve({
      handler: async (request) => {
        await request.text();
        headers = request.headers;
        promise.resolve();
        return new Response("");
      },
      port: 2333,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const conn = await Deno.connect({ port: 2333 });
    // Send GET request with a body + content-length.
    const encoder = new TextEncoder();
    const body =
      `GET / HTTP/1.1\r\nHost: 127.0.0.1:2333\r\nContent-Length: 5\r\n\r\n12345`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);
    await promise;
    conn.close();
    assertEquals(headers!.get("content-length"), "5");
    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpReadHeadersAfterClose() {
    const promise = deferred();
    const ac = new AbortController();
    const listeningPromise = deferred();

    let req: Request;
    const server = Deno.serve({
      handler: async (request) => {
        await request.text();
        req = request;
        promise.resolve();
        return new Response("Hello World");
      },
      port: 2334,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const conn = await Deno.connect({ port: 2334 });
    // Send GET request with a body + content-length.
    const encoder = new TextEncoder();
    const body =
      `GET / HTTP/1.1\r\nHost: 127.0.0.1:2333\r\nContent-Length: 5\r\n\r\n12345`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);
    await promise;
    conn.close();

    assertThrows(
      () => {
        req.headers;
      },
      TypeError,
      "request closed",
    );

    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerGetRequestBody() {
    const promise = deferred();
    const ac = new AbortController();
    const listeningPromise = deferred();

    const server = Deno.serve({
      handler: (request) => {
        assertEquals(request.body, null);
        promise.resolve();
        return new Response("", { headers: {} });
      },
      port: 4501,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const conn = await Deno.connect({ port: 4501 });
    // Send GET request with a body + content-length.
    const encoder = new TextEncoder();
    const body =
      `GET / HTTP/1.1\r\nHost: 127.0.0.1:4501\r\nContent-Length: 5\r\n\r\n12345`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);

    const resp = new Uint8Array(200);
    const readResult = await conn.read(resp);
    assert(readResult);
    assert(readResult > 0);

    conn.close();
    await promise;
    ac.abort();
    await server;
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

    const listeningPromise = deferred();
    const ac = new AbortController();
    const server = Deno.serve({
      handler: (request) => {
        assert(!request.body);
        return new Response(stream.readable);
      },
      port: 4501,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const resp = await fetch("http://127.0.0.1:4501/");
    const respBody = await resp.text();
    assertEquals("hello world", respBody);
    ac.abort();
    await server;
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
    const listeningPromise = deferred();
    const ac = new AbortController();
    const server = Deno.serve({
      handler: async (request) => {
        const reqBody = await request.text();
        assertEquals("hello world", reqBody);
        return new Response("yo");
      },
      port: 4501,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const resp = await fetch("http://127.0.0.1:4501/", {
      body: stream.readable,
      method: "POST",
      headers: { "connection": "close" },
    });

    assertEquals(await resp.text(), "yo");
    ac.abort();
    await server;
  },
);

Deno.test({ permissions: { net: true } }, async function httpServerClose() {
  const ac = new AbortController();
  const listeningPromise = deferred();
  const server = Deno.serve({
    handler: () => new Response("ok"),
    port: 4501,
    signal: ac.signal,
    onListen: onListen(listeningPromise),
    onError: createOnErrorCb(ac),
  });
  await listeningPromise;
  const client = await Deno.connect({ port: 4501 });
  client.close();
  ac.abort();
  await server;
});

// FIXME:
Deno.test(
  { permissions: { net: true } },
  async function httpServerEmptyBlobResponse() {
    const ac = new AbortController();
    const listeningPromise = deferred();
    const server = Deno.serve({
      handler: () => new Response(new Blob([])),
      port: 4501,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const resp = await fetch("http://127.0.0.1:4501/");
    const respBody = await resp.text();

    assertEquals("", respBody);
    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerCorrectLengthForUnicodeString() {
    const ac = new AbortController();
    const listeningPromise = deferred();

    const server = Deno.serve({
      handler: () => new Response("韓國".repeat(10)),
      port: 4503,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const conn = await Deno.connect({ port: 4503 });
    const encoder = new TextEncoder();
    const decoder = new TextDecoder();

    const body =
      `GET / HTTP/1.1\r\nHost: example.domain\r\nConnection: close\r\n\r\n`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);

    const buf = new Uint8Array(1024);
    const readResult = await conn.read(buf);
    assert(readResult);
    const msg = decoder.decode(buf.subarray(0, readResult));

    conn.close();

    ac.abort();
    await server;
    assert(msg.includes("Content-Length: 60"));
  },
);

Deno.test({ permissions: { net: true } }, async function httpServerWebSocket() {
  const ac = new AbortController();
  const listeningPromise = deferred();
  const server = Deno.serve({
    handler: async (request) => {
      const {
        response,
        socket,
      } = Deno.upgradeWebSocket(request);
      socket.onerror = () => fail();
      socket.onmessage = (m) => {
        socket.send(m.data);
        socket.close(1001);
      };
      return response;
    },
    port: 4501,
    signal: ac.signal,
    onListen: onListen(listeningPromise),
    onError: createOnErrorCb(ac),
  });

  await listeningPromise;
  const def = deferred();
  const ws = new WebSocket("ws://localhost:4501");
  ws.onmessage = (m) => assertEquals(m.data, "foo");
  ws.onerror = () => fail();
  ws.onclose = () => def.resolve();
  ws.onopen = () => ws.send("foo");

  await def;
  ac.abort();
  await server;
});

Deno.test(
  { permissions: { net: true } },
  async function httpVeryLargeRequest() {
    const promise = deferred();
    const listeningPromise = deferred();
    const ac = new AbortController();

    let headers: Headers;
    const server = Deno.serve({
      handler: async (request) => {
        headers = request.headers;
        promise.resolve();
        return new Response("");
      },
      port: 2333,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const conn = await Deno.connect({ port: 2333 });
    // Send GET request with a body + content-length.
    const encoder = new TextEncoder();
    const smthElse = "x".repeat(16 * 1024 + 256);
    const body =
      `GET / HTTP/1.1\r\nHost: 127.0.0.1:2333\r\nContent-Length: 5\r\nSomething-Else: ${smthElse}\r\n\r\n`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);
    await promise;
    conn.close();
    assertEquals(headers!.get("content-length"), "5");
    assertEquals(headers!.get("something-else"), smthElse);
    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpVeryLargeRequestAndBody() {
    const promise = deferred();
    const listeningPromise = deferred();
    const ac = new AbortController();

    let headers: Headers;
    let text: string;
    const server = Deno.serve({
      handler: async (request) => {
        headers = request.headers;
        text = await request.text();
        promise.resolve();
        return new Response("");
      },
      port: 2333,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const conn = await Deno.connect({ port: 2333 });
    // Send GET request with a body + content-length.
    const encoder = new TextEncoder();
    const smthElse = "x".repeat(16 * 1024 + 256);
    const reqBody = "hello world".repeat(1024);
    let body =
      `PUT / HTTP/1.1\r\nHost: 127.0.0.1:2333\r\nContent-Length: ${reqBody.length}\r\nSomething-Else: ${smthElse}\r\n\r\n${reqBody}`;

    while (body.length > 0) {
      const writeResult = await conn.write(encoder.encode(body));
      body = body.slice(writeResult);
    }

    await promise;
    conn.close();

    assertEquals(headers!.get("content-length"), `${reqBody.length}`);
    assertEquals(headers!.get("something-else"), smthElse);
    assertEquals(text!, reqBody);
    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpConnectionClose() {
    const promise = deferred();
    const ac = new AbortController();
    const listeningPromise = deferred();

    const server = Deno.serve({
      handler: () => {
        promise.resolve();
        return new Response("");
      },
      port: 2333,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const conn = await Deno.connect({ port: 2333 });
    // Send GET request with a body + connection: close.
    const encoder = new TextEncoder();
    const body =
      `GET / HTTP/1.1\r\nHost: 127.0.0.1:2333\r\nConnection: Close\r\n\r\n`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);

    await promise;
    conn.close();

    ac.abort();
    await server;
  },
);

// FIXME: auto request body reading is intefering with passing it as response.
// Deno.test(
//   { permissions: { net: true } },
//   async function httpServerStreamDuplex() {
//     const promise = deferred();
//     const ac = new AbortController();

//     const server = Deno.serve(request => {
//       assert(request.body);

//       promise.resolve();
//       return new Response(request.body);
//     }, { port: 2333, signal: ac.signal });

//     const ts = new TransformStream();
//     const writable = ts.writable.getWriter();

//     const resp = await fetch("http://127.0.0.1:2333/", {
//       method: "POST",
//       body: ts.readable,
//     });

//     await promise;
//     assert(resp.body);
//     const reader = resp.body.getReader();
//     await writable.write(new Uint8Array([1]));
//     const chunk1 = await reader.read();
//     assert(!chunk1.done);
//     assertEquals(chunk1.value, new Uint8Array([1]));
//     await writable.write(new Uint8Array([2]));
//     const chunk2 = await reader.read();
//     assert(!chunk2.done);
//     assertEquals(chunk2.value, new Uint8Array([2]));
//     await writable.close();
//     const chunk3 = await reader.read();
//     assert(chunk3.done);

//     ac.abort();
//     await server;
//   },
// );

Deno.test(
  { permissions: { net: true } },
  // Issue: https://github.com/denoland/deno/issues/10930
  async function httpServerStreamingResponse() {
    // This test enqueues a single chunk for readable
    // stream and waits for client to read that chunk and signal
    // it before enqueueing subsequent chunk. Issue linked above
    // presented a situation where enqueued chunks were not
    // written to the HTTP connection until the next chunk was enqueued.
    const listeningPromise = deferred();
    const promise = deferred();
    const ac = new AbortController();

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

      try {
        while ((result = await chunkedReader.read(buf)) !== null) {
          const len = Math.min(buf.byteLength, result);

          await dest.write(buf.subarray(0, len));

          // Resolve a deferred - this will make response stream to
          // enqueue next chunk.
          deferreds[counter - 1].resolve();
        }
        return decoder.decode(dest.bytes());
      } catch (e) {
        console.error(e);
      }
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

    const finished = Deno.serve({
      handler: () => {
        promise.resolve();
        return new Response(periodicStream());
      },
      port: 4501,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    // start a client
    const clientConn = await Deno.connect({ port: 4501 });

    const r1 = await writeRequest(clientConn);
    assertEquals(r1, "0\n1\n2\n");

    ac.abort();
    await promise;
    await finished;
    clientConn.close();
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpRequestLatin1Headers() {
    const listeningPromise = deferred();
    const promise = deferred();
    const ac = new AbortController();
    const server = Deno.serve({
      handler: (request) => {
        assertEquals(request.headers.get("X-Header-Test"), "á");
        promise.resolve();
        return new Response("hello", { headers: { "X-Header-Test": "Æ" } });
      },
      port: 4501,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
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

    const buf = new Uint8Array(1024);
    await clientConn.read(buf);

    await promise;
    let responseText = new TextDecoder().decode(buf);
    clientConn.close();

    assert(/\r\n[Xx]-[Hh]eader-[Tt]est: Æ\r\n/.test(responseText));

    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerRequestWithoutPath() {
    const promise = deferred();
    const listeningPromise = deferred();
    const ac = new AbortController();

    const server = Deno.serve({
      handler: async (request) => {
        // FIXME:
        // assertEquals(new URL(request.url).href, "http://127.0.0.1:4501/");
        assertEquals(await request.text(), "");
        promise.resolve();
        return new Response("11");
      },
      port: 4501,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
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

    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpCookieConcatenation() {
    const promise = deferred();
    const listeningPromise = deferred();
    const ac = new AbortController();

    const server = Deno.serve({
      handler: async (request) => {
        assertEquals(await request.text(), "");
        assertEquals(request.headers.get("cookie"), "foo=bar, bar=foo");
        promise.resolve();
        return new Response("ok");
      },
      port: 4501,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const resp = await fetch("http://127.0.0.1:4501/", {
      headers: [
        ["connection", "close"],
        ["cookie", "foo=bar"],
        ["cookie", "bar=foo"],
      ],
    });
    await promise;

    const text = await resp.text();
    assertEquals(text, "ok");

    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true, write: true, read: true } },
  async function httpServerCorrectSizeResponse() {
    const promise = deferred();
    const listeningPromise = deferred();
    const ac = new AbortController();

    const tmpFile = await Deno.makeTempFile();
    const file = await Deno.open(tmpFile, { write: true, read: true });
    await file.write(new Uint8Array(70 * 1024).fill(1)); // 70kb sent in 64kb + 6kb chunks
    file.close();

    const server = Deno.serve({
      handler: async (request) => {
        const f = await Deno.open(tmpFile, { read: true });
        promise.resolve();
        return new Response(f.readable);
      },
      port: 4503,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const resp = await fetch("http://127.0.0.1:4503/");
    await promise;
    const body = await resp.arrayBuffer();

    assertEquals(body.byteLength, 70 * 1024);
    ac.abort();
    await server;
  },
);

// https://github.com/denoland/deno/issues/12741
// https://github.com/denoland/deno/pull/12746
// https://github.com/denoland/deno/pull/12798
Deno.test(
  { permissions: { net: true, run: true } },
  async function httpServerDeleteRequestHasBody() {
    const promise = deferred();
    const listeningPromise = deferred();
    const ac = new AbortController();

    const hostname = "localhost";
    const port = 4501;

    const server = Deno.serve({
      handler: () => {
        promise.resolve();
        return new Response("ok");
      },
      port: port,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const url = `http://${hostname}:${port}/`;
    const args = ["-X", "DELETE", url];
    const { success } = await Deno.spawn("curl", {
      args,
      stdout: "null",
      stderr: "null",
    });
    assert(success);
    await promise;
    ac.abort();

    await server;
  },
);

// FIXME:
Deno.test(
  { permissions: { net: true } },
  async function httpServerRespondNonAsciiUint8Array() {
    const promise = deferred();
    const listeningPromise = deferred();
    const ac = new AbortController();

    const server = Deno.serve({
      handler: async (request) => {
        assertEquals(request.body, null);
        promise.resolve();
        return new Response(new Uint8Array([128]));
      },
      port: 4501,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });
    await listeningPromise;
    const resp = await fetch("http://localhost:4501/");

    await promise;

    assertEquals(resp.status, 200);
    const body = await resp.arrayBuffer();
    assertEquals(new Uint8Array(body), new Uint8Array([128]));

    ac.abort();
    await server;
  },
);

Deno.test("upgradeHttpRaw tcp", async () => {
  const promise = deferred();
  const listeningPromise = deferred();
  const promise2 = deferred();
  const ac = new AbortController();
  const signal = ac.signal;
  const handler = async (req: Request) => {
    const [conn, _] = Deno.upgradeHttpRaw(req);

    await conn.write(
      new TextEncoder().encode("HTTP/1.1 101 Switching Protocols\r\n\r\n"),
    );

    promise.resolve();

    const buf = new Uint8Array(1024);
    const n = await conn.read(buf);

    assert(n != null);
    const secondPacketText = new TextDecoder().decode(buf.slice(0, n));
    assertEquals(secondPacketText, "bla bla bla\nbla bla\nbla\n");

    promise2.resolve();
    conn.close();
  };
  const server = Deno.serve({
    // NOTE: `as any` is used to bypass type checking for the return value
    // of the handler.
    handler: handler as any,
    port: 4501,
    signal,
    onListen: onListen(listeningPromise),
    onError: createOnErrorCb(ac),
  });

  await listeningPromise;
  const tcpConn = await Deno.connect({ port: 4501 });
  await tcpConn.write(
    new TextEncoder().encode(
      "CONNECT server.example.com:80 HTTP/1.1\r\n\r\n",
    ),
  );

  await promise;

  await tcpConn.write(
    new TextEncoder().encode(
      "bla bla bla\nbla bla\nbla\n",
    ),
  );

  await promise2;
  tcpConn.close();

  ac.abort();
  await server;
});

// Some of these tests are ported from Hyper
// https://github.com/hyperium/hyper/blob/889fa2d87252108eb7668b8bf034ffcc30985117/src/proto/h1/role.rs
// https://github.com/hyperium/hyper/blob/889fa2d87252108eb7668b8bf034ffcc30985117/tests/server.rs

Deno.test(
  { permissions: { net: true } },
  async function httpServerParseRequest() {
    const promise = deferred();
    const listeningPromise = deferred();
    const ac = new AbortController();

    const server = Deno.serve({
      handler: async (request) => {
        assertEquals(request.method, "GET");
        assertEquals(request.headers.get("host"), "deno.land");
        promise.resolve();
        return new Response("ok");
      },
      port: 4503,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const conn = await Deno.connect({ port: 4503 });
    const encoder = new TextEncoder();
    const body = `GET /echo HTTP/1.1\r\nHost: deno.land\r\n\r\n`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);
    await promise;
    conn.close();

    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerParseHeaderHtabs() {
    const promise = deferred();
    const listeningPromise = deferred();
    const ac = new AbortController();

    const server = Deno.serve({
      handler: async (request) => {
        assertEquals(request.method, "GET");
        assertEquals(request.headers.get("server"), "hello\tworld");
        promise.resolve();
        return new Response("ok");
      },
      port: 4503,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const conn = await Deno.connect({ port: 4503 });
    const encoder = new TextEncoder();
    const body = `GET / HTTP/1.1\r\nserver: hello\tworld\r\n\r\n`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);
    await promise;
    conn.close();

    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerGetShouldIgnoreBody() {
    const promise = deferred();
    const listeningPromise = deferred();
    const ac = new AbortController();

    const server = Deno.serve({
      handler: async (request) => {
        assertEquals(request.method, "GET");
        assertEquals(await request.text(), "");
        promise.resolve();
        return new Response("ok");
      },
      port: 4503,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const conn = await Deno.connect({ port: 4503 });
    const encoder = new TextEncoder();
    // Connection: close = don't try to parse the body as a new request
    const body =
      `GET / HTTP/1.1\r\nHost: example.domain\r\nConnection: close\r\n\r\nI shouldn't be read.\r\n`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);
    await promise;
    conn.close();

    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerPostWithBody() {
    const promise = deferred();
    const listeningPromise = deferred();
    const ac = new AbortController();

    const server = Deno.serve({
      handler: async (request) => {
        assertEquals(request.method, "POST");
        assertEquals(await request.text(), "I'm a good request.");
        promise.resolve();
        return new Response("ok");
      },
      port: 4503,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const conn = await Deno.connect({ port: 4503 });
    const encoder = new TextEncoder();
    const body =
      `POST / HTTP/1.1\r\nHost: example.domain\r\nContent-Length: 19\r\n\r\nI'm a good request.`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);
    await promise;
    conn.close();

    ac.abort();
    await server;
  },
);

type TestCase = {
  headers?: Record<string, string>;
  body: any;
  expects_chunked?: boolean;
  expects_con_len?: boolean;
};

function hasHeader(msg: string, name: string): boolean {
  let n = msg.indexOf("\r\n\r\n") || msg.length;
  return msg.slice(0, n).includes(name);
}

function createServerLengthTest(name: string, testCase: TestCase) {
  Deno.test(name, async function () {
    const promise = deferred();
    const ac = new AbortController();
    const listeningPromise = deferred();

    const server = Deno.serve({
      handler: async (request) => {
        assertEquals(request.method, "GET");
        promise.resolve();
        return new Response(testCase.body, testCase.headers ?? {});
      },
      port: 4503,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const conn = await Deno.connect({ port: 4503 });
    const encoder = new TextEncoder();
    const body =
      `GET / HTTP/1.1\r\nHost: example.domain\r\nConnection: close\r\n\r\n`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);
    await promise;

    const decoder = new TextDecoder();
    let msg = "";
    while (true) {
      const buf = new Uint8Array(1024);
      const readResult = await conn.read(buf);
      if (!readResult) {
        break;
      }
      msg += decoder.decode(buf.subarray(0, readResult));
      try {
        assert(
          testCase.expects_chunked == hasHeader(msg, "Transfer-Encoding:"),
        );
        assert(testCase.expects_chunked == hasHeader(msg, "chunked"));
        assert(testCase.expects_con_len == hasHeader(msg, "Content-Length:"));

        const n = msg.indexOf("\r\n\r\n") + 4;

        if (testCase.expects_chunked) {
          assertEquals(msg.slice(n + 1, n + 3), "\r\n");
          assertEquals(msg.slice(msg.length - 7), "\r\n0\r\n\r\n");
        }

        if (testCase.expects_con_len && typeof testCase.body === "string") {
          assertEquals(msg.slice(n), testCase.body);
        }
        break;
      } catch (e) {
        continue;
      }
    }

    conn.close();

    ac.abort();
    await server;
  });
}

// Quick and dirty way to make a readable stream from a string. Alternatively,
// `readableStreamFromReader(file)` could be used.
function stream(s: string): ReadableStream<Uint8Array> {
  return new Response(s).body!;
}

createServerLengthTest("fixedResponseKnown", {
  headers: { "content-length": "11" },
  body: "foo bar baz",
  expects_chunked: false,
  expects_con_len: true,
});

createServerLengthTest("fixedResponseUnknown", {
  headers: { "content-length": "11" },
  body: stream("foo bar baz"),
  expects_chunked: true,
  expects_con_len: false,
});

createServerLengthTest("fixedResponseKnownEmpty", {
  headers: { "content-length": "0" },
  body: "",
  expects_chunked: false,
  expects_con_len: true,
});

createServerLengthTest("chunkedRespondKnown", {
  headers: { "transfer-encoding": "chunked" },
  body: "foo bar baz",
  expects_chunked: false,
  expects_con_len: true,
});

createServerLengthTest("chunkedRespondUnknown", {
  headers: { "transfer-encoding": "chunked" },
  body: stream("foo bar baz"),
  expects_chunked: true,
  expects_con_len: false,
});

createServerLengthTest("autoResponseWithKnownLength", {
  body: "foo bar baz",
  expects_chunked: false,
  expects_con_len: true,
});

createServerLengthTest("autoResponseWithUnknownLength", {
  body: stream("foo bar baz"),
  expects_chunked: true,
  expects_con_len: false,
});

createServerLengthTest("autoResponseWithKnownLengthEmpty", {
  body: "",
  expects_chunked: false,
  expects_con_len: true,
});

createServerLengthTest("autoResponseWithUnknownLengthEmpty", {
  body: stream(""),
  expects_chunked: true,
  expects_con_len: false,
});

Deno.test(
  {
    // FIXME(bartlomieju): this test is hanging on Windows, needs to be
    // investigated and fixed
    ignore: Deno.build.os === "windows",
    permissions: { net: true },
  },
  async function httpServerGetChunkedResponseWithKa() {
    const promises = [deferred(), deferred()];
    let reqCount = 0;
    const listeningPromise = deferred();
    const ac = new AbortController();

    const server = Deno.serve({
      handler: async (request) => {
        assertEquals(request.method, "GET");
        promises[reqCount].resolve();
        reqCount++;
        return new Response(reqCount <= 1 ? stream("foo bar baz") : "zar quux");
      },
      port: 4503,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const conn = await Deno.connect({ port: 4503 });
    const encoder = new TextEncoder();
    {
      const body =
        `GET / HTTP/1.1\r\nHost: example.domain\r\nConnection: keep-alive\r\n\r\n`;
      const writeResult = await conn.write(encoder.encode(body));
      assertEquals(body.length, writeResult);
      await promises[0];
    }

    const decoder = new TextDecoder();
    {
      let msg = "";
      while (true) {
        try {
          const buf = new Uint8Array(1024);
          const readResult = await conn.read(buf);
          assert(readResult);
          msg += decoder.decode(buf.subarray(0, readResult));
          assert(msg.endsWith("\r\nfoo bar baz\r\n0\r\n\r\n"));
          break;
        } catch {
          continue;
        }
      }
    }

    // once more!
    {
      const body =
        `GET /quux HTTP/1.1\r\nHost: example.domain\r\nConnection: close\r\n\r\n`;
      const writeResult = await conn.write(encoder.encode(body));
      assertEquals(body.length, writeResult);
      await promises[1];
    }
    {
      const buf = new Uint8Array(1024);
      const readResult = await conn.read(buf);
      assert(readResult);
      const msg = decoder.decode(buf.subarray(0, readResult));
      assert(msg.endsWith("zar quux"));
    }

    conn.close();

    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerPostWithContentLengthBody() {
    const promise = deferred();
    const listeningPromise = deferred();
    const ac = new AbortController();

    const server = Deno.serve({
      handler: async (request) => {
        assertEquals(request.method, "POST");
        assertEquals(request.headers.get("content-length"), "5");
        assertEquals(await request.text(), "hello");
        promise.resolve();
        return new Response("ok");
      },
      port: 4503,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const conn = await Deno.connect({ port: 4503 });
    const encoder = new TextEncoder();

    const body =
      `POST / HTTP/1.1\r\nHost: example.domain\r\nContent-Length: 5\r\n\r\nhello`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);
    await promise;

    conn.close();

    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerPostWithInvalidPrefixContentLength() {
    const ac = new AbortController();
    const listeningPromise = deferred();
    const server = Deno.serve({
      handler: () => {
        throw new Error("unreachable");
      },
      port: 4503,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const conn = await Deno.connect({ port: 4503 });
    const encoder = new TextEncoder();
    const decoder = new TextDecoder();

    const body =
      `POST / HTTP/1.1\r\nHost: example.domain\r\nContent-Length: +5\r\n\r\nhello`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);

    const buf = new Uint8Array(1024);
    const readResult = await conn.read(buf);
    assert(readResult);
    const msg = decoder.decode(buf.subarray(0, readResult));
    assert(msg.endsWith("HTTP/1.1 400 Bad Request\r\n\r\n"));

    conn.close();

    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerPostWithChunkedBody() {
    const promise = deferred();
    const ac = new AbortController();
    const listeningPromise = deferred();

    const server = Deno.serve({
      handler: async (request) => {
        assertEquals(request.method, "POST");
        assertEquals(await request.text(), "qwert");
        promise.resolve();
        return new Response("ok");
      },
      port: 4503,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const conn = await Deno.connect({ port: 4503 });
    const encoder = new TextEncoder();

    const body =
      `POST / HTTP/1.1\r\nHost: example.domain\r\nTransfer-Encoding: chunked\r\n\r\n1\r\nq\r\n2\r\nwe\r\n2\r\nrt\r\n0\r\n\r\n`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);
    await promise;

    conn.close();

    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerPostWithIncompleteBody() {
    const promise = deferred();
    const ac = new AbortController();
    const listeningPromise = deferred();

    const server = Deno.serve({
      handler: async (r) => {
        promise.resolve();
        assertEquals(await r.text(), "12345");
        return new Response("ok");
      },
      port: 4503,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const conn = await Deno.connect({ port: 4503 });
    const encoder = new TextEncoder();

    const body =
      `POST / HTTP/1.1\r\nHost: example.domain\r\nContent-Length: 10\r\n\r\n12345`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);

    await promise;
    conn.close();

    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerHeadResponseDoesntSendBody() {
    const promise = deferred();
    const ac = new AbortController();
    const listeningPromise = deferred();

    const server = Deno.serve({
      handler: () => {
        promise.resolve();
        return new Response("foo bar baz");
      },
      port: 4503,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const conn = await Deno.connect({ port: 4503 });
    const encoder = new TextEncoder();
    const decoder = new TextDecoder();

    const body =
      `HEAD / HTTP/1.1\r\nHost: example.domain\r\nConnection: close\r\n\r\n`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);

    await promise;

    const buf = new Uint8Array(1024);
    const readResult = await conn.read(buf);
    assert(readResult);
    const msg = decoder.decode(buf.subarray(0, readResult));

    assert(msg.endsWith("Content-Length: 11\r\n\r\n"));

    conn.close();

    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true, write: true, read: true } },
  async function httpServerSendFile() {
    const promise = deferred();
    const ac = new AbortController();
    const listeningPromise = deferred();
    const tmpFile = await Deno.makeTempFile();
    const file = await Deno.open(tmpFile, { write: true, read: true });
    const data = new Uint8Array(70 * 1024).fill(1);
    await file.write(data);
    file.close();
    const server = Deno.serve({
      handler: async () => {
        const f = await Deno.open(tmpFile, { read: true });
        promise.resolve();
        return new Response(f.readable, { status: 200 });
      },
      port: 4503,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const response = await fetch(`http://localhost:4503/`);
    assertEquals(response.status, 200);
    await promise;
    assertEquals(new Uint8Array(await response.arrayBuffer()), data);
    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true, write: true, read: true } },
  async function httpServerPostFile() {
    const promise = deferred();
    const ac = new AbortController();
    const listeningPromise = deferred();

    const tmpFile = await Deno.makeTempFile();
    const file = await Deno.open(tmpFile, { write: true, read: true });
    const data = new Uint8Array(70 * 1024).fill(1);
    await file.write(data);
    file.close();

    const server = Deno.serve({
      handler: async (request) => {
        assertEquals(new Uint8Array(await request.arrayBuffer()), data);
        promise.resolve();
        return new Response("ok");
      },
      port: 4503,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const f = await Deno.open(tmpFile, { write: true, read: true });
    const response = await fetch(`http://localhost:4503/`, {
      method: "POST",
      body: f.readable,
    });

    await promise;

    assertEquals(response.status, 200);
    assertEquals(await response.text(), "ok");

    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function httpServerWithTls() {
    const ac = new AbortController();
    const listeningPromise = deferred();
    const hostname = "127.0.0.1";
    const port = 4501;

    const server = Deno.serve({
      handler: () => new Response("Hello World"),
      hostname,
      port,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
      cert: Deno.readTextFileSync("cli/tests/testdata/tls/localhost.crt"),
      key: Deno.readTextFileSync("cli/tests/testdata/tls/localhost.key"),
    });

    await listeningPromise;
    const caCert = Deno.readTextFileSync("cli/tests/testdata/tls/RootCA.pem");
    const client = Deno.createHttpClient({ caCerts: [caCert] });
    const resp = await fetch(`https://localhost:${port}/`, {
      client,
      headers: { "connection": "close" },
    });

    const respBody = await resp.text();
    assertEquals("Hello World", respBody);

    client.close();
    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true, write: true, read: true } },
  async function httpServerRequestCLTE() {
    const ac = new AbortController();
    const listeningPromise = deferred();
    const promise = deferred();

    const server = Deno.serve({
      handler: async (req) => {
        assertEquals(await req.text(), "");
        promise.resolve();
        return new Response("ok");
      },
      port: 4503,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const conn = await Deno.connect({ port: 4503 });
    const encoder = new TextEncoder();

    const body =
      `POST / HTTP/1.1\r\nHost: example.domain\r\nContent-Length: 13\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\nEXTRA`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);
    await promise;

    conn.close();

    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true, write: true, read: true } },
  async function httpServerRequestTETE() {
    const ac = new AbortController();
    const listeningPromise = deferred();

    const server = Deno.serve({
      handler: () => {
        throw new Error("oops");
      },
      port: 4503,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    const encoder = new TextEncoder();
    const decoder = new TextDecoder();

    const variations = [
      "Transfer-Encoding : chunked",
      "Transfer-Encoding: xchunked",
      "Transfer-Encoding: chunkedx",
      "Transfer-Encoding\n: chunked",
    ];

    await listeningPromise;
    for (const teHeader of variations) {
      const conn = await Deno.connect({ port: 4503 });
      const body =
        `POST / HTTP/1.1\r\nHost: example.domain\r\n${teHeader}\r\n\r\n0\r\n\r\n`;
      const writeResult = await conn.write(encoder.encode(body));
      assertEquals(body.length, writeResult);

      const buf = new Uint8Array(1024);
      const readResult = await conn.read(buf);
      assert(readResult);
      const msg = decoder.decode(buf.subarray(0, readResult));
      assert(msg.endsWith("HTTP/1.1 400 Bad Request\r\n\r\n"));

      conn.close();
    }

    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServer304ResponseDoesntSendBody() {
    const promise = deferred();
    const ac = new AbortController();
    const listeningPromise = deferred();

    const server = Deno.serve({
      handler: () => {
        promise.resolve();
        return new Response(null, { status: 304 });
      },
      port: 4503,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const conn = await Deno.connect({ port: 4503 });
    const encoder = new TextEncoder();
    const decoder = new TextDecoder();

    const body =
      `GET / HTTP/1.1\r\nHost: example.domain\r\nConnection: close\r\n\r\n`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);

    await promise;

    const buf = new Uint8Array(1024);
    const readResult = await conn.read(buf);
    assert(readResult);
    const msg = decoder.decode(buf.subarray(0, readResult));

    assert(msg.startsWith("HTTP/1.1 304 Not Modified"));
    assert(msg.endsWith("\r\n\r\n"));

    conn.close();

    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerExpectContinue() {
    const promise = deferred();
    const ac = new AbortController();
    const listeningPromise = deferred();

    const server = Deno.serve({
      handler: async (req) => {
        promise.resolve();
        assertEquals(await req.text(), "hello");
        return new Response(null, { status: 304 });
      },
      port: 4503,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const conn = await Deno.connect({ port: 4503 });
    const encoder = new TextEncoder();
    const decoder = new TextDecoder();

    {
      const body =
        `POST / HTTP/1.1\r\nHost: example.domain\r\nExpect: 100-continue\r\nContent-Length: 5\r\nConnection: close\r\n\r\n`;
      const writeResult = await conn.write(encoder.encode(body));
      assertEquals(body.length, writeResult);
    }

    await promise;

    {
      const msgExpected = "HTTP/1.1 100 Continue\r\n\r\n";
      const buf = new Uint8Array(encoder.encode(msgExpected).byteLength);
      const readResult = await conn.read(buf);
      assert(readResult);
      const msg = decoder.decode(buf.subarray(0, readResult));
      assert(msg.startsWith(msgExpected));
    }

    {
      const body = "hello";
      const writeResult = await conn.write(encoder.encode(body));
      assertEquals(body.length, writeResult);
    }

    const buf = new Uint8Array(1024);
    const readResult = await conn.read(buf);
    assert(readResult);
    const msg = decoder.decode(buf.subarray(0, readResult));

    assert(msg.startsWith("HTTP/1.1 304 Not Modified"));
    conn.close();

    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerExpectContinueButNoBodyLOL() {
    const promise = deferred();
    const listeningPromise = deferred();
    const ac = new AbortController();

    const server = Deno.serve({
      handler: async (req) => {
        promise.resolve();
        assertEquals(await req.text(), "");
        return new Response(null, { status: 304 });
      },
      port: 4503,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const conn = await Deno.connect({ port: 4503 });
    const encoder = new TextEncoder();
    const decoder = new TextDecoder();

    {
      // // no content-length or transfer-encoding means no body!
      const body =
        `POST / HTTP/1.1\r\nHost: example.domain\r\nExpect: 100-continue\r\nConnection: close\r\n\r\n`;
      const writeResult = await conn.write(encoder.encode(body));
      assertEquals(body.length, writeResult);
    }

    await promise;

    const buf = new Uint8Array(1024);
    const readResult = await conn.read(buf);
    assert(readResult);
    const msg = decoder.decode(buf.subarray(0, readResult));

    assert(msg.startsWith("HTTP/1.1 304 Not Modified"));
    conn.close();

    ac.abort();
    await server;
  },
);

const badRequests = [
  ["weirdMethodName", "GE T / HTTP/1.1\r\n\r\n"],
  ["illegalRequestLength", "POST / HTTP/1.1\r\nContent-Length: foo\r\n\r\n"],
  ["illegalRequestLength2", "POST / HTTP/1.1\r\nContent-Length: -1\r\n\r\n"],
  ["illegalRequestLength3", "POST / HTTP/1.1\r\nContent-Length: 1.1\r\n\r\n"],
  ["illegalRequestLength4", "POST / HTTP/1.1\r\nContent-Length: 1.\r\n\r\n"],
];

for (const [name, req] of badRequests) {
  const testFn = {
    [name]: async () => {
      const ac = new AbortController();
      const listeningPromise = deferred();

      const server = Deno.serve({
        handler: () => {
          throw new Error("oops");
        },
        port: 4503,
        signal: ac.signal,
        onListen: onListen(listeningPromise),
        onError: createOnErrorCb(ac),
      });

      await listeningPromise;
      const conn = await Deno.connect({ port: 4503 });
      const encoder = new TextEncoder();
      const decoder = new TextDecoder();

      {
        const writeResult = await conn.write(encoder.encode(req));
        assertEquals(req.length, writeResult);
      }

      const buf = new Uint8Array(100);
      const readResult = await conn.read(buf);
      assert(readResult);
      const msg = decoder.decode(buf.subarray(0, readResult));

      assert(msg.startsWith("HTTP/1.1 400 "));
      conn.close();

      ac.abort();
      await server;
    },
  }[name];

  Deno.test(
    { permissions: { net: true } },
    testFn,
  );
}

Deno.test(
  { permissions: { net: true } },
  async function httpServerImplicitZeroContentLengthForHead() {
    const ac = new AbortController();
    const listeningPromise = deferred();

    const server = Deno.serve({
      handler: () => new Response(null),
      port: 4503,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const conn = await Deno.connect({ port: 4503 });
    const encoder = new TextEncoder();
    const decoder = new TextDecoder();

    const body =
      `HEAD / HTTP/1.1\r\nHost: example.domain\r\nConnection: close\r\n\r\n`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);

    const buf = new Uint8Array(1024);
    const readResult = await conn.read(buf);
    assert(readResult);
    const msg = decoder.decode(buf.subarray(0, readResult));

    assert(msg.includes("Content-Length: 0"));

    conn.close();

    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerConcurrentRequests() {
    const ac = new AbortController();
    const listeningPromise = deferred();

    let reqCount = -1;
    let timerId: number | undefined;
    const server = Deno.serve({
      handler: async (req) => {
        reqCount++;
        if (reqCount === 0) {
          const msg = new TextEncoder().encode("data: hello\r\n\r\n");
          // SSE
          const body = new ReadableStream({
            start(controller) {
              timerId = setInterval(() => {
                controller.enqueue(msg);
              }, 1000);
            },
            cancel() {
              if (typeof timerId === "number") {
                clearInterval(timerId);
              }
            },
          });
          return new Response(body, {
            headers: {
              "Content-Type": "text/event-stream",
            },
          });
        }

        return new Response(`hello ${reqCount}`);
      },
      port: 4503,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    const sseRequest = await fetch(`http://localhost:4503/`);

    const decoder = new TextDecoder();
    const stream = sseRequest.body!.getReader();
    {
      const { done, value } = await stream.read();
      assert(!done);
      assertEquals(decoder.decode(value), "data: hello\r\n\r\n");
    }

    const helloRequest = await fetch(`http://localhost:4503/`);
    assertEquals(helloRequest.status, 200);
    assertEquals(await helloRequest.text(), "hello 1");

    {
      const { done, value } = await stream.read();
      assert(!done);
      assertEquals(decoder.decode(value), "data: hello\r\n\r\n");
    }

    await stream.cancel();
    clearInterval(timerId);
    ac.abort();
    await server;
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
