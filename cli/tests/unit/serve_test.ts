// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertMatch } from "https://deno.land/std@v0.42.0/testing/asserts.ts";
import { Buffer, BufReader, BufWriter } from "../../../test_util/std/io/mod.ts";
import { TextProtoReader } from "../testdata/run/textproto.ts";
import {
  assert,
  assertEquals,
  assertStringIncludes,
  assertThrows,
  Deferred,
  deferred,
  execCode,
  fail,
} from "./test_util.ts";

// Since these tests may run in parallel, ensure this port is unique to this file
const servePort = 4502;

const {
  upgradeHttpRaw,
  addTrailers,
  // @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
} = Deno[Deno.internal];

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

Deno.test(async function httpServerShutsDownPortBeforeResolving() {
  const ac = new AbortController();
  const listeningPromise = deferred();

  const server = Deno.serve({
    handler: (_req) => new Response("ok"),
    port: servePort,
    signal: ac.signal,
    onListen: onListen(listeningPromise),
  });

  await listeningPromise;
  assertThrows(() => Deno.listen({ port: servePort }));

  ac.abort();
  await server.finished;

  const listener = Deno.listen({ port: servePort });
  listener!.close();
});

Deno.test(
  { permissions: { read: true, run: true } },
  async function httpServerUnref() {
    const [statusCode, _output] = await execCode(`
      async function main() {
        const server = Deno.serve({ port: 4501, handler: () => null });
        server.unref();
        await server.finished; // This doesn't block the program from exiting
      }
      main();
    `);
    assertEquals(statusCode, 0);
  },
);

Deno.test(async function httpServerCanResolveHostnames() {
  const ac = new AbortController();
  const listeningPromise = deferred();

  const server = Deno.serve({
    handler: (_req) => new Response("ok"),
    hostname: "localhost",
    port: servePort,
    signal: ac.signal,
    onListen: onListen(listeningPromise),
    onError: createOnErrorCb(ac),
  });

  await listeningPromise;
  const resp = await fetch(`http://localhost:${servePort}/`, {
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
    port: servePort,
    signal: ac.signal,
    onListen: onListen(listeningPromise),
    onError: createOnErrorCb(ac),
  });
  await listeningPromise;

  assertThrows(
    () =>
      Deno.serve({
        handler: (_req) => new Response("ok"),
        hostname: "localhost",
        port: servePort,
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
    handler: async (request, { remoteAddr }) => {
      // FIXME(bartlomieju):
      // make sure that request can be inspected
      console.log(request);
      assertEquals(new URL(request.url).href, `http://127.0.0.1:${servePort}/`);
      assertEquals(await request.text(), "");
      assertEquals(remoteAddr.hostname, "127.0.0.1");
      promise.resolve();
      return new Response("Hello World", { headers: { "foo": "bar" } });
    },
    port: servePort,
    signal: ac.signal,
    onListen: onListen(listeningPromise),
    onError: createOnErrorCb(ac),
  });

  await listeningPromise;
  const resp = await fetch(`http://127.0.0.1:${servePort}/`, {
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

Deno.test({ permissions: { net: true } }, async function httpServerOnError() {
  const ac = new AbortController();
  const listeningPromise = deferred();
  let requestStash: Request | null;

  const server = Deno.serve({
    handler: async (request: Request) => {
      requestStash = request;
      await new Promise((r) => setTimeout(r, 100));
      throw "fail";
    },
    port: servePort,
    signal: ac.signal,
    onListen: onListen(listeningPromise),
    onError: () => {
      return new Response("failed: " + requestStash!.url, { status: 500 });
    },
  });

  await listeningPromise;
  const resp = await fetch(`http://127.0.0.1:${servePort}/`, {
    headers: { "connection": "close" },
  });
  const text = await resp.text();
  ac.abort();
  await server;

  assertEquals(text, `failed: http://127.0.0.1:${servePort}/`);
});

Deno.test(
  { permissions: { net: true } },
  async function httpServerOnErrorFails() {
    const ac = new AbortController();
    const listeningPromise = deferred();
    // NOTE(bartlomieju): deno lint doesn't know that it's actually used later,
    // but TypeScript can't see that either ¯\_(ツ)_/¯
    // deno-lint-ignore no-unused-vars
    let requestStash: Request | null;

    const server = Deno.serve({
      handler: async (request: Request) => {
        requestStash = request;
        await new Promise((r) => setTimeout(r, 100));
        throw "fail";
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: () => {
        throw "again";
      },
    });

    await listeningPromise;
    const resp = await fetch(`http://127.0.0.1:${servePort}/`, {
      headers: { "connection": "close" },
    });
    const text = await resp.text();
    ac.abort();
    await server;

    assertEquals(text, "Internal Server Error");
  },
);

Deno.test({ permissions: { net: true } }, async function httpServerOverload1() {
  const ac = new AbortController();
  const promise = deferred();
  const listeningPromise = deferred();

  const server = Deno.serve({
    port: servePort,
    signal: ac.signal,
    onListen: onListen(listeningPromise),
    onError: createOnErrorCb(ac),
  }, async (request) => {
    // FIXME(bartlomieju):
    // make sure that request can be inspected
    console.log(request);
    assertEquals(new URL(request.url).href, `http://127.0.0.1:${servePort}/`);
    assertEquals(await request.text(), "");
    promise.resolve();
    return new Response("Hello World", { headers: { "foo": "bar" } });
  });

  await listeningPromise;
  const resp = await fetch(`http://127.0.0.1:${servePort}/`, {
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

  const server = Deno.serve({
    port: servePort,
    signal: ac.signal,
    onListen: onListen(listeningPromise),
    onError: createOnErrorCb(ac),
  }, async (request) => {
    // FIXME(bartlomieju):
    // make sure that request can be inspected
    console.log(request);
    assertEquals(new URL(request.url).href, `http://127.0.0.1:${servePort}/`);
    assertEquals(await request.text(), "");
    promise.resolve();
    return new Response("Hello World", { headers: { "foo": "bar" } });
  });

  await listeningPromise;
  const resp = await fetch(`http://127.0.0.1:${servePort}/`, {
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
  function httpServerErrorOverloadMissingHandler() {
    // @ts-ignore - testing invalid overload
    assertThrows(() => Deno.serve(), TypeError, "handler");
    // @ts-ignore - testing invalid overload
    assertThrows(() => Deno.serve({}), TypeError, "handler");
    assertThrows(
      // @ts-ignore - testing invalid overload
      () => Deno.serve({ handler: undefined }),
      TypeError,
      "handler",
    );
    assertThrows(
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
        assert(!!match, `Didn't match ${msg}`);
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
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const conn = await Deno.connect({ port: servePort });
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

function createUrlTest(
  name: string,
  methodAndPath: string,
  host: string | null,
  expected: string,
) {
  Deno.test(`httpServerUrl${name}`, async () => {
    const listeningPromise: Deferred<number> = deferred();
    const urlPromise = deferred();
    const ac = new AbortController();
    const server = Deno.serve({
      handler: (request: Request) => {
        urlPromise.resolve(request.url);
        return new Response("");
      },
      port: 0,
      signal: ac.signal,
      onListen: ({ port }: { port: number }) => {
        listeningPromise.resolve(port);
      },
      onError: createOnErrorCb(ac),
    });

    const port = await listeningPromise;
    const conn = await Deno.connect({ port });

    const encoder = new TextEncoder();
    const body = `${methodAndPath} HTTP/1.1\r\n${
      host ? ("Host: " + host + "\r\n") : ""
    }Content-Length: 5\r\n\r\n12345`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);

    try {
      const expectedResult = expected.replace("HOST", "localhost").replace(
        "PORT",
        `${port}`,
      );
      assertEquals(await urlPromise, expectedResult);
    } finally {
      ac.abort();
      await server;
      conn.close();
    }
  });
}

createUrlTest("WithPath", "GET /path", null, "http://HOST:PORT/path");
createUrlTest(
  "WithPathAndHost",
  "GET /path",
  "deno.land",
  "http://deno.land/path",
);
createUrlTest(
  "WithAbsolutePath",
  "GET http://localhost/path",
  null,
  "http://localhost/path",
);
createUrlTest(
  "WithAbsolutePathAndHost",
  "GET http://localhost/path",
  "deno.land",
  "http://localhost/path",
);
createUrlTest(
  "WithPortAbsolutePath",
  "GET http://localhost:1234/path",
  null,
  "http://localhost:1234/path",
);
createUrlTest(
  "WithPortAbsolutePathAndHost",
  "GET http://localhost:1234/path",
  "deno.land",
  "http://localhost:1234/path",
);
createUrlTest(
  "WithPortAbsolutePathAndHostWithPort",
  "GET http://localhost:1234/path",
  "deno.land:9999",
  "http://localhost:1234/path",
);

createUrlTest("WithAsterisk", "OPTIONS *", null, "*");
createUrlTest(
  "WithAuthorityForm",
  "CONNECT deno.land:80",
  null,
  "deno.land:80",
);

// TODO(mmastrac): These should probably be 400 errors
createUrlTest("WithInvalidAsterisk", "GET *", null, "*");
createUrlTest("WithInvalidNakedPath", "GET path", null, "path");
createUrlTest(
  "WithInvalidNakedAuthority",
  "GET deno.land:1234",
  null,
  "deno.land:1234",
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
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const conn = await Deno.connect({ port: servePort });
    // Send GET request with a body + content-length.
    const encoder = new TextEncoder();
    const body =
      `GET / HTTP/1.1\r\nHost: 127.0.0.1:${servePort}\r\nContent-Length: 5\r\n\r\n12345`;
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

function createStreamTest(count: number, delay: number, action: string) {
  function doAction(controller: ReadableStreamDefaultController, i: number) {
    if (i == count) {
      if (action == "Throw") {
        controller.error(new Error("Expected error!"));
      } else {
        controller.close();
      }
    } else {
      controller.enqueue(`a${i}`);

      if (delay == 0) {
        doAction(controller, i + 1);
      } else {
        setTimeout(() => doAction(controller, i + 1), delay);
      }
    }
  }

  function makeStream(_count: number, delay: number): ReadableStream {
    return new ReadableStream({
      start(controller) {
        if (delay == 0) {
          doAction(controller, 0);
        } else {
          setTimeout(() => doAction(controller, 0), delay);
        }
      },
    }).pipeThrough(new TextEncoderStream());
  }

  Deno.test(`httpServerStreamCount${count}Delay${delay}${action}`, async () => {
    const ac = new AbortController();
    const listeningPromise = deferred();
    const server = Deno.serve({
      handler: (_request) => {
        return new Response(makeStream(count, delay));
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const resp = await fetch(`http://127.0.0.1:${servePort}/`);
    const text = await resp.text();

    ac.abort();
    await server;
    let expected = "";
    if (action == "Throw" && count < 2 && delay < 1000) {
      // NOTE: This is specific to the current implementation. In some cases where a stream errors, we
      // don't send the first packet.
      expected = "";
    } else {
      for (let i = 0; i < count; i++) {
        expected += `a${i}`;
      }
    }

    assertEquals(text, expected);
  });
}

for (const count of [0, 1, 2, 3]) {
  for (const delay of [0, 1, 1000]) {
    // Creating a stream that errors in start will throw
    if (delay > 0) {
      createStreamTest(count, delay, "Throw");
    }
    createStreamTest(count, delay, "Close");
  }
}

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
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const resp = await fetch(`http://127.0.0.1:${servePort}/`, {
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
    port: servePort,
    signal: ac.signal,
    onListen: onListen(listeningPromise),
    onError: createOnErrorCb(ac),
  });
  await listeningPromise;
  const client = await Deno.connect({ port: servePort });
  client.close();
  ac.abort();
  await server;
});

// https://github.com/denoland/deno/issues/15427
Deno.test({ permissions: { net: true } }, async function httpServerCloseGet() {
  const ac = new AbortController();
  const listeningPromise = deferred();
  const requestPromise = deferred();
  const responsePromise = deferred();
  const server = Deno.serve({
    handler: async () => {
      requestPromise.resolve();
      await new Promise((r) => setTimeout(r, 500));
      responsePromise.resolve();
      return new Response("ok");
    },
    port: servePort,
    signal: ac.signal,
    onListen: onListen(listeningPromise),
    onError: createOnErrorCb(ac),
  });
  await listeningPromise;
  const conn = await Deno.connect({ port: servePort });
  const encoder = new TextEncoder();
  const body =
    `GET / HTTP/1.1\r\nHost: example.domain\r\nConnection: close\r\n\r\n`;
  const writeResult = await conn.write(encoder.encode(body));
  assertEquals(body.length, writeResult);
  await requestPromise;
  conn.close();
  await responsePromise;
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
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const resp = await fetch(`http://127.0.0.1:${servePort}/`);
    const respBody = await resp.text();

    assertEquals("", respBody);
    ac.abort();
    await server;
  },
);

// https://github.com/denoland/deno/issues/17291
Deno.test(
  { permissions: { net: true } },
  async function httpServerIncorrectChunkedResponse() {
    const ac = new AbortController();
    const listeningPromise = deferred();
    const errorPromise = deferred();
    const server = Deno.serve({
      handler: () => {
        const body = new ReadableStream({
          start(controller) {
            // Non-encoded string is not a valid readable chunk.
            // @ts-ignore we're testing that input is invalid
            controller.enqueue("wat");
          },
          type: "bytes",
        });
        return new Response(body);
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: (err) => {
        const errResp = new Response(
          `Internal server error: ${(err as Error).message}`,
          { status: 500 },
        );
        errorPromise.resolve();
        return errResp;
      },
    });

    await listeningPromise;
    const resp = await fetch(`http://127.0.0.1:${servePort}/`);
    // Incorrectly implemented reader ReadableStream should reject.
    assertStringIncludes(await resp.text(), "Failed to execute 'enqueue'");
    await errorPromise;
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
    assert(msg.includes("content-length: 60"));
  },
);

Deno.test({ permissions: { net: true } }, async function httpServerWebSocket() {
  const ac = new AbortController();
  const listeningPromise = deferred();
  const server = Deno.serve({
    handler: (request) => {
      const {
        response,
        socket,
      } = Deno.upgradeWebSocket(request);
      socket.onerror = (e) => {
        console.error(e);
        fail();
      };
      socket.onmessage = (m) => {
        socket.send(m.data);
        socket.close(1001);
      };
      return response;
    },
    port: servePort,
    signal: ac.signal,
    onListen: onListen(listeningPromise),
    onError: createOnErrorCb(ac),
  });

  await listeningPromise;
  const def = deferred();
  const ws = new WebSocket(`ws://localhost:${servePort}`);
  ws.onmessage = (m) => assertEquals(m.data, "foo");
  ws.onerror = (e) => {
    console.error(e);
    fail();
  };
  ws.onclose = () => def.resolve();
  ws.onopen = () => ws.send("foo");

  await def;
  ac.abort();
  await server;
});

Deno.test(
  { permissions: { net: true } },
  async function httpServerWebSocketRaw() {
    const ac = new AbortController();
    const listeningPromise = deferred();
    const server = Deno.serve({
      handler: async (request) => {
        const { conn, response } = upgradeHttpRaw(request);
        const buf = new Uint8Array(1024);
        let read;

        // Write our fake HTTP upgrade
        await conn.write(
          new TextEncoder().encode(
            "HTTP/1.1 101 Switching Protocols\r\nConnection: Upgraded\r\n\r\nExtra",
          ),
        );

        // Upgrade data
        read = await conn.read(buf);
        assertEquals(
          new TextDecoder().decode(buf.subarray(0, read!)),
          "Upgrade data",
        );
        // Read the packet to echo
        read = await conn.read(buf);
        // Echo
        await conn.write(buf.subarray(0, read!));

        conn.close();
        return response;
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;

    const conn = await Deno.connect({ port: servePort });
    await conn.write(
      new TextEncoder().encode(
        "GET / HTTP/1.1\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n\r\nUpgrade data",
      ),
    );
    const buf = new Uint8Array(1024);
    let len;

    // Headers
    let headers = "";
    for (let i = 0; i < 2; i++) {
      len = await conn.read(buf);
      headers += new TextDecoder().decode(buf.subarray(0, len!));
      if (headers.endsWith("Extra")) {
        break;
      }
    }
    assertMatch(
      headers,
      /HTTP\/1\.1 101 Switching Protocols[ ,.A-Za-z:0-9\r\n]*Extra/im,
    );

    // Data to echo
    await conn.write(new TextEncoder().encode("buffer data"));

    // Echo
    len = await conn.read(buf);
    assertEquals(
      new TextDecoder().decode(buf.subarray(0, len!)),
      "buffer data",
    );

    conn.close();
    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerWebSocketUpgradeTwice() {
    const ac = new AbortController();
    const listeningPromise = deferred();
    const server = Deno.serve({
      handler: (request) => {
        const {
          response,
          socket,
        } = Deno.upgradeWebSocket(request);
        assertThrows(
          () => {
            Deno.upgradeWebSocket(request);
          },
          Deno.errors.Http,
          "already upgraded",
        );
        socket.onerror = (e) => {
          console.error(e);
          fail();
        };
        socket.onmessage = (m) => {
          socket.send(m.data);
          socket.close(1001);
        };
        return response;
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const def = deferred();
    const ws = new WebSocket(`ws://localhost:${servePort}`);
    ws.onmessage = (m) => assertEquals(m.data, "foo");
    ws.onerror = (e) => {
      console.error(e);
      fail();
    };
    ws.onclose = () => def.resolve();
    ws.onopen = () => ws.send("foo");

    await def;
    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerWebSocketCloseFast() {
    const ac = new AbortController();
    const listeningPromise = deferred();
    const server = Deno.serve({
      handler: (request) => {
        const {
          response,
          socket,
        } = Deno.upgradeWebSocket(request);
        socket.onopen = () => socket.close();
        return response;
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const def = deferred();
    const ws = new WebSocket(`ws://localhost:${servePort}`);
    ws.onerror = (e) => {
      console.error(e);
      fail();
    };
    ws.onclose = () => def.resolve();

    await def;
    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerWebSocketCanAccessRequest() {
    const ac = new AbortController();
    const listeningPromise = deferred();
    const server = Deno.serve({
      handler: (request) => {
        const {
          response,
          socket,
        } = Deno.upgradeWebSocket(request);
        socket.onerror = (e) => {
          console.error(e);
          fail();
        };
        socket.onmessage = (_m) => {
          socket.send(request.url.toString());
          socket.close(1001);
        };
        return response;
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const def = deferred();
    const ws = new WebSocket(`ws://localhost:${servePort}`);
    ws.onmessage = (m) =>
      assertEquals(m.data, `http://localhost:${servePort}/`);
    ws.onerror = (e) => {
      console.error(e);
      fail();
    };
    ws.onclose = () => def.resolve();
    ws.onopen = () => ws.send("foo");

    await def;
    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpVeryLargeRequest() {
    const promise = deferred();
    const listeningPromise = deferred();
    const ac = new AbortController();

    let headers: Headers;
    const server = Deno.serve({
      handler: (request) => {
        headers = request.headers;
        promise.resolve();
        return new Response("");
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const conn = await Deno.connect({ port: servePort });
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
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const conn = await Deno.connect({ port: servePort });
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
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const conn = await Deno.connect({ port: servePort });
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

Deno.test(
  { permissions: { net: true } },
  async function httpServerStreamDuplex() {
    const promise = deferred();
    const ac = new AbortController();

    const server = Deno.serve(
      { port: servePort, signal: ac.signal },
      (request) => {
        assert(request.body);

        promise.resolve();
        return new Response(request.body);
      },
    );

    const ts = new TransformStream();
    const writable = ts.writable.getWriter();

    const resp = await fetch(`http://127.0.0.1:${servePort}/`, {
      method: "POST",
      body: ts.readable,
    });

    await promise;
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

    ac.abort();
    await server;
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
      const body = `GET / HTTP/1.1\r\nHost: 127.0.0.1:${servePort}\r\n\r\n`;
      const writeResult = await w.write(encoder.encode(body));
      assertEquals(body.length, writeResult);
      await w.flush();
      const tpr = new TextProtoReader(r);
      const statusLine = await tpr.readLine();
      assert(statusLine !== null);
      const headers = await tpr.readMimeHeader();
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
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    // start a client
    const clientConn = await Deno.connect({ port: servePort });

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
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const clientConn = await Deno.connect({ port: servePort });
    const requestText =
      `GET / HTTP/1.1\r\nHost: 127.0.0.1:${servePort}\r\nX-Header-Test: á\r\n\r\n`;
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
    const responseText = new TextDecoder("iso-8859-1").decode(buf);
    clientConn.close();

    ac.abort();
    await server;

    assertMatch(responseText, /\r\n[Xx]-[Hh]eader-[Tt]est: Æ\r\n/);
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
        // assertEquals(new URL(request.url).href, `http://127.0.0.1:${servePort}/`);
        assertEquals(await request.text(), "");
        promise.resolve();
        return new Response("11");
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const clientConn = await Deno.connect({ port: servePort });

    async function writeRequest(conn: Deno.Conn) {
      const encoder = new TextEncoder();

      const w = new BufWriter(conn);
      const r = new BufReader(conn);
      const body =
        `CONNECT 127.0.0.1:${servePort} HTTP/1.1\r\nHost: 127.0.0.1:${servePort}\r\n\r\n`;
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
      const headers = await tpr.readMimeHeader();
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
        assertEquals(request.headers.get("cookie"), "foo=bar; bar=foo");
        promise.resolve();
        return new Response("ok");
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
      reusePort: true,
    });

    await listeningPromise;
    const resp = await fetch(`http://127.0.0.1:${servePort}/`, {
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

    const server = Deno.serve({
      handler: () => {
        promise.resolve();
        return new Response("ok");
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const url = `http://${hostname}:${servePort}/`;
    const args = ["-X", "DELETE", url];
    const { success } = await new Deno.Command("curl", {
      args,
      stdout: "null",
      stderr: "null",
    }).output();
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
      handler: (request) => {
        assertEquals(request.body, null);
        promise.resolve();
        return new Response(new Uint8Array([128]));
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });
    await listeningPromise;
    const resp = await fetch(`http://localhost:${servePort}/`);

    await promise;

    assertEquals(resp.status, 200);
    const body = await resp.arrayBuffer();
    assertEquals(new Uint8Array(body), new Uint8Array([128]));

    ac.abort();
    await server;
  },
);

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
      handler: (request) => {
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
      handler: (request) => {
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
  // deno-lint-ignore no-explicit-any
  body: any;
  expectsChunked?: boolean;
  expectsConnLen?: boolean;
};

function hasHeader(msg: string, name: string): boolean {
  const n = msg.indexOf("\r\n\r\n") || msg.length;
  return msg.slice(0, n).includes(name);
}

function createServerLengthTest(name: string, testCase: TestCase) {
  Deno.test(name, async function () {
    const promise = deferred();
    const ac = new AbortController();
    const listeningPromise = deferred();

    const server = Deno.serve({
      handler: (request) => {
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
          testCase.expectsChunked == hasHeader(msg, "Transfer-Encoding:"),
        );
        assert(testCase.expectsChunked == hasHeader(msg, "chunked"));
        assert(testCase.expectsConnLen == hasHeader(msg, "Content-Length:"));

        const n = msg.indexOf("\r\n\r\n") + 4;

        if (testCase.expectsChunked) {
          assertEquals(msg.slice(n + 1, n + 3), "\r\n");
          assertEquals(msg.slice(msg.length - 7), "\r\n0\r\n\r\n");
        }

        if (testCase.expectsConnLen && typeof testCase.body === "string") {
          assertEquals(msg.slice(n), testCase.body);
        }
        break;
      } catch {
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
  expectsChunked: false,
  expectsConnLen: true,
});

createServerLengthTest("fixedResponseUnknown", {
  headers: { "content-length": "11" },
  body: stream("foo bar baz"),
  expectsChunked: true,
  expectsConnLen: false,
});

createServerLengthTest("fixedResponseKnownEmpty", {
  headers: { "content-length": "0" },
  body: "",
  expectsChunked: false,
  expectsConnLen: true,
});

createServerLengthTest("chunkedRespondKnown", {
  headers: { "transfer-encoding": "chunked" },
  body: "foo bar baz",
  expectsChunked: false,
  expectsConnLen: true,
});

createServerLengthTest("chunkedRespondUnknown", {
  headers: { "transfer-encoding": "chunked" },
  body: stream("foo bar baz"),
  expectsChunked: true,
  expectsConnLen: false,
});

createServerLengthTest("autoResponseWithKnownLength", {
  body: "foo bar baz",
  expectsChunked: false,
  expectsConnLen: true,
});

createServerLengthTest("autoResponseWithUnknownLength", {
  body: stream("foo bar baz"),
  expectsChunked: true,
  expectsConnLen: false,
});

createServerLengthTest("autoResponseWithKnownLengthEmpty", {
  body: "",
  expectsChunked: false,
  expectsConnLen: true,
});

createServerLengthTest("autoResponseWithUnknownLengthEmpty", {
  body: stream(""),
  expectsChunked: true,
  expectsConnLen: false,
});

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
    assert(msg.includes("HTTP/1.1 400 Bad Request"));

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
        return new Response("NaN".repeat(100));
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

    assert(msg.includes("content-length: 300\r\n"));

    conn.close();

    ac.abort();
    await server;
  },
);

function makeTempData(size: number) {
  return new Uint8Array(size).fill(1);
}

async function makeTempFile(size: number) {
  const tmpFile = await Deno.makeTempFile();
  const file = await Deno.open(tmpFile, { write: true, read: true });
  const data = makeTempData(size);
  await file.write(data);
  file.close();

  return await Deno.open(tmpFile, { write: true, read: true });
}

const compressionTestCases = [
  { name: "Empty", length: 0, in: {}, out: {}, expect: null },
  {
    name: "EmptyAcceptGzip",
    length: 0,
    in: { "Accept-Encoding": "gzip" },
    out: {},
    expect: null,
  },
  // This technically would be compressible if not for the size, however the size_hint is not implemented
  // for FileResource and we don't currently peek ahead on resources.
  // {
  //   name: "EmptyAcceptGzip2",
  //   length: 0,
  //   in: { "Accept-Encoding": "gzip" },
  //   out: { "Content-Type": "text/plain" },
  //   expect: null,
  // },
  { name: "Uncompressible", length: 1024, in: {}, out: {}, expect: null },
  {
    name: "UncompressibleAcceptGzip",
    length: 1024,
    in: { "Accept-Encoding": "gzip" },
    out: {},
    expect: null,
  },
  {
    name: "UncompressibleType",
    length: 1024,
    in: { "Accept-Encoding": "gzip" },
    out: { "Content-Type": "text/fake" },
    expect: null,
  },
  {
    name: "CompressibleType",
    length: 1024,
    in: { "Accept-Encoding": "gzip" },
    out: { "Content-Type": "text/plain" },
    expect: "gzip",
  },
  {
    name: "CompressibleType2",
    length: 1024,
    in: { "Accept-Encoding": "gzip, deflate, br" },
    out: { "Content-Type": "text/plain" },
    expect: "gzip",
  },
  {
    name: "UncompressibleRange",
    length: 1024,
    in: { "Accept-Encoding": "gzip" },
    out: { "Content-Type": "text/plain", "Content-Range": "1" },
    expect: null,
  },
  {
    name: "UncompressibleCE",
    length: 1024,
    in: { "Accept-Encoding": "gzip" },
    out: { "Content-Type": "text/plain", "Content-Encoding": "random" },
    expect: null,
  },
  {
    name: "UncompressibleCC",
    length: 1024,
    in: { "Accept-Encoding": "gzip" },
    out: { "Content-Type": "text/plain", "Cache-Control": "no-transform" },
    expect: null,
  },
];

for (const testCase of compressionTestCases) {
  const name = `httpServerCompression${testCase.name}`;
  Deno.test(
    { permissions: { net: true, write: true, read: true } },
    {
      [name]: async function () {
        const promise = deferred();
        const ac = new AbortController();
        const listeningPromise = deferred();
        const server = Deno.serve({
          handler: async (_request) => {
            const f = await makeTempFile(testCase.length);
            promise.resolve();
            // deno-lint-ignore no-explicit-any
            const headers = testCase.out as any;
            headers["Content-Length"] = testCase.length.toString();
            return new Response(f.readable, {
              headers: headers as HeadersInit,
            });
          },
          port: 4503,
          signal: ac.signal,
          onListen: onListen(listeningPromise),
          onError: createOnErrorCb(ac),
        });
        try {
          await listeningPromise;
          const resp = await fetch("http://127.0.0.1:4503/", {
            headers: testCase.in as HeadersInit,
          });
          await promise;
          const body = await resp.arrayBuffer();
          if (testCase.expect == null) {
            assertEquals(body.byteLength, testCase.length);
            assertEquals(
              resp.headers.get("content-length"),
              testCase.length.toString(),
            );
            assertEquals(
              resp.headers.get("content-encoding"),
              testCase.out["Content-Encoding"] || null,
            );
          } else if (testCase.expect == "gzip") {
            // Note the fetch will transparently decompress this response, BUT we can detect that a response
            // was compressed by the lack of a content length.
            assertEquals(body.byteLength, testCase.length);
            assertEquals(resp.headers.get("content-encoding"), null);
            assertEquals(resp.headers.get("content-length"), null);
          }
        } finally {
          ac.abort();
          await server;
        }
      },
    }[name],
  );
}

Deno.test(
  { permissions: { net: true, write: true, read: true } },
  async function httpServerPostFile() {
    const promise = deferred();
    const ac = new AbortController();
    const listeningPromise = deferred();

    const server = Deno.serve({
      handler: async (request) => {
        assertEquals(
          new Uint8Array(await request.arrayBuffer()),
          makeTempData(70 * 1024),
        );
        promise.resolve();
        return new Response("ok");
      },
      port: 4503,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    await listeningPromise;
    const f = await makeTempFile(70 * 1024);
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

    const server = Deno.serve({
      handler: () => new Response("Hello World"),
      hostname,
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
      cert: Deno.readTextFileSync("cli/tests/testdata/tls/localhost.crt"),
      key: Deno.readTextFileSync("cli/tests/testdata/tls/localhost.key"),
    });

    await listeningPromise;
    const caCert = Deno.readTextFileSync("cli/tests/testdata/tls/RootCA.pem");
    const client = Deno.createHttpClient({ caCerts: [caCert] });
    const resp = await fetch(`https://localhost:${servePort}/`, {
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
      assert(msg.includes("HTTP/1.1 400 Bad Request\r\n"));

      conn.close();
    }

    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServer204ResponseDoesntSendContentLength() {
    const listeningPromise = deferred();
    const ac = new AbortController();
    const server = Deno.serve({
      handler: (_request) => new Response(null, { status: 204 }),
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    try {
      await listeningPromise;
      const resp = await fetch(`http://127.0.0.1:${servePort}/`, {
        method: "GET",
        headers: { "connection": "close" },
      });
      assertEquals(resp.status, 204);
      assertEquals(resp.headers.get("Content-Length"), null);
    } finally {
      ac.abort();
      await server;
    }
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
  async function httpServerConcurrentRequests() {
    const ac = new AbortController();
    const listeningPromise = deferred();

    let reqCount = -1;
    let timerId: number | undefined;
    const server = Deno.serve({
      handler: (_req) => {
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

Deno.test(
  { permissions: { net: true } },
  async function serveWithPrototypePollution() {
    const originalThen = Promise.prototype.then;
    const originalSymbolIterator = Array.prototype[Symbol.iterator];
    try {
      Promise.prototype.then = Array.prototype[Symbol.iterator] = () => {
        throw new Error();
      };
      const ac = new AbortController();
      const listeningPromise = deferred();
      const server = Deno.serve({
        handler: (_req) => new Response("ok"),
        hostname: "localhost",
        port: servePort,
        signal: ac.signal,
        onListen: onListen(listeningPromise),
        onError: createOnErrorCb(ac),
      });
      ac.abort();
      await server;
    } finally {
      Promise.prototype.then = originalThen;
      Array.prototype[Symbol.iterator] = originalSymbolIterator;
    }
  },
);

// https://github.com/denoland/deno/issues/15549
Deno.test(
  { permissions: { net: true } },
  async function testIssue15549() {
    const ac = new AbortController();
    const promise = deferred();
    let count = 0;
    const server = Deno.serve({
      async onListen({ port }: { port: number }) {
        const res1 = await fetch(`http://localhost:${port}/`);
        assertEquals(await res1.text(), "hello world 1");

        const res2 = await fetch(`http://localhost:${port}/`);
        assertEquals(await res2.text(), "hello world 2");

        promise.resolve();
        ac.abort();
      },
      signal: ac.signal,
    }, () => {
      count++;
      return new Response(`hello world ${count}`);
    });

    await promise;
    await server;
  },
);

// https://github.com/denoland/deno/issues/15858
Deno.test(
  { permissions: { net: true } },
  async function httpServerCanCloneRequest() {
    const ac = new AbortController();
    const listeningPromise = deferred();

    const server = Deno.serve({
      handler: async (req) => {
        const cloned = req.clone();
        assertEquals(req.headers, cloned.headers);

        // both requests can read body
        await req.text();
        await cloned.json();

        return new Response("ok");
      },
      signal: ac.signal,
      onListen: ({ port }: { port: number }) => listeningPromise.resolve(port),
      onError: createOnErrorCb(ac),
    });

    try {
      const port = await listeningPromise;
      const resp = await fetch(`http://localhost:${port}/`, {
        headers: { connection: "close" },
        method: "POST",
        body: '{"sus":true}',
      });
      const text = await resp.text();
      assertEquals(text, "ok");
    } finally {
      ac.abort();
      await server;
    }
  },
);

// Checks large streaming response
// https://github.com/denoland/deno/issues/16567
Deno.test(
  { permissions: { net: true } },
  async function testIssue16567() {
    const ac = new AbortController();
    const promise = deferred();
    const server = Deno.serve({
      async onListen({ port }) {
        const res1 = await fetch(`http://localhost:${port}/`);
        assertEquals((await res1.text()).length, 40 * 50_000);

        promise.resolve();
        ac.abort();
      },
      signal: ac.signal,
    }, () =>
      new Response(
        new ReadableStream({
          start(c) {
            // 2MB "a...a" response with 40 chunks
            for (const _ of Array(40)) {
              c.enqueue(new Uint8Array(50_000).fill(97));
            }
            c.close();
          },
        }),
      ));

    await promise;
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
  const result = await tp.readMimeHeader();
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

Deno.test(
  { permissions: { net: true, run: true } },
  async function httpServeCurlH2C() {
    const ac = new AbortController();
    const server = Deno.serve(
      { signal: ac.signal },
      () => new Response("hello world!"),
    );

    assertEquals(
      "hello world!",
      await curlRequest(["http://localhost:8000/path"]),
    );
    assertEquals(
      "hello world!",
      await curlRequest(["http://localhost:8000/path", "--http2"]),
    );
    assertEquals(
      "hello world!",
      await curlRequest([
        "http://localhost:8000/path",
        "--http2",
        "--http2-prior-knowledge",
      ]),
    );

    ac.abort();
    await server;
  },
);

// TODO(mmastrac): This test should eventually use fetch, when we support trailers there.
// This test is ignored because it's flaky and relies on cURL's verbose output.
Deno.test(
  { permissions: { net: true, run: true, read: true }, ignore: true },
  async function httpServerTrailers() {
    const ac = new AbortController();
    const listeningPromise = deferred();

    const server = Deno.serve({
      handler: () => {
        const response = new Response("Hello World", {
          headers: {
            "trailer": "baz",
            "transfer-encoding": "chunked",
            "foo": "bar",
          },
        });
        addTrailers(response, [["baz", "why"]]);
        return response;
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningPromise),
      onError: createOnErrorCb(ac),
    });

    // We don't have a great way to access this right now, so just fetch the trailers with cURL
    const [_, stderr] = await curlRequestWithStdErr([
      `http://localhost:${servePort}/path`,
      "-v",
      "--http2",
      "--http2-prior-knowledge",
    ]);
    assertMatch(stderr, /baz: why/);
    ac.abort();
    await server;
  },
);

Deno.test(
  { permissions: { net: true, run: true, read: true } },
  async function httpsServeCurlH2C() {
    const ac = new AbortController();
    const server = Deno.serve(
      {
        signal: ac.signal,
        cert: Deno.readTextFileSync("cli/tests/testdata/tls/localhost.crt"),
        key: Deno.readTextFileSync("cli/tests/testdata/tls/localhost.key"),
      },
      () => new Response("hello world!"),
    );

    assertEquals(
      "hello world!",
      await curlRequest(["https://localhost:9000/path", "-k"]),
    );
    assertEquals(
      "hello world!",
      await curlRequest(["https://localhost:9000/path", "-k", "--http2"]),
    );
    assertEquals(
      "hello world!",
      await curlRequest([
        "https://localhost:9000/path",
        "-k",
        "--http2",
        "--http2-prior-knowledge",
      ]),
    );

    ac.abort();
    await server;
  },
);

async function curlRequest(args: string[]) {
  const { success, stdout } = await new Deno.Command("curl", {
    args,
    stdout: "piped",
    stderr: "null",
  }).output();
  assert(success);
  return new TextDecoder().decode(stdout);
}

async function curlRequestWithStdErr(args: string[]) {
  const { success, stdout, stderr } = await new Deno.Command("curl", {
    args,
    stdout: "piped",
    stderr: "piped",
  }).output();
  assert(success);
  return [new TextDecoder().decode(stdout), new TextDecoder().decode(stderr)];
}
