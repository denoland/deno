// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { assertMatch, assertRejects } from "@std/assert";
import { Buffer, BufReader, BufWriter } from "@std/io";
import { TextProtoReader } from "../testdata/run/textproto.ts";
import {
  assert,
  assertEquals,
  assertStringIncludes,
  assertThrows,
  curlRequest,
  curlRequestWithStdErr,
  execCode,
  execCode3,
  fail,
  tmpUnixSocketPath,
} from "./test_util.ts";

// Since these tests may run in parallel, ensure this port is unique to this file
const servePort = 4502;

const {
  upgradeHttpRaw,
  addTrailers,
  serveHttpOnListener,
  serveHttpOnConnection,
  getCachedAbortSignal,
  // @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
} = Deno[Deno.internal];

function createOnErrorCb(ac: AbortController): (err: unknown) => Response {
  return (err) => {
    console.error(err);
    ac.abort();
    return new Response("Internal server error", { status: 500 });
  };
}

function onListen(
  resolve: (value: void | PromiseLike<void>) => void,
): ({ hostname, port }: { hostname: string; port: number }) => void {
  return () => {
    resolve();
  };
}

async function makeServer(
  handler: (
    req: Request,
    info: Deno.ServeHandlerInfo,
  ) => Response | Promise<Response>,
): Promise<
  {
    finished: Promise<void>;
    abort: () => void;
    shutdown: () => Promise<void>;
    [Symbol.asyncDispose](): PromiseLike<void>;
  }
> {
  const ac = new AbortController();
  const { promise, resolve } = Promise.withResolvers<void>();

  const server = Deno.serve({
    handler,
    port: servePort,
    signal: ac.signal,
    onListen: onListen(resolve),
  });

  await promise;
  return {
    finished: server.finished,
    abort() {
      ac.abort();
    },
    async shutdown() {
      await server.shutdown();
    },
    [Symbol.asyncDispose]() {
      return server[Symbol.asyncDispose]();
    },
  };
}

Deno.test(async function httpServerShutsDownPortBeforeResolving() {
  const { finished, abort } = await makeServer((_req) => new Response("ok"));
  assertThrows(() => Deno.listen({ port: servePort }));
  abort();
  await finished;

  const listener = Deno.listen({ port: servePort });
  listener!.close();
});

// When shutting down abruptly, we require that all in-progress connections are aborted,
// no new connections are allowed, and no new transactions are allowed on existing connections.
Deno.test(
  { permissions: { net: true } },
  async function httpServerShutdownAbruptGuaranteeHttp11() {
    const deferredQueue: {
      input: ReturnType<typeof Promise.withResolvers<string>>;
      out: ReturnType<typeof Promise.withResolvers<void>>;
    }[] = [];
    const { finished, abort } = await makeServer((_req) => {
      const { input, out } = deferredQueue.shift()!;
      return new Response(
        new ReadableStream({
          async start(controller) {
            controller.enqueue(new Uint8Array([46]));
            out.resolve();
            controller.enqueue(encoder.encode(await input.promise));
            controller.close();
          },
        }),
      );
    });
    const encoder = new TextEncoder();
    const decoder = new TextDecoder();
    const conn = await Deno.connect({ port: servePort });
    const w = conn.writable.getWriter();
    const r = conn.readable.getReader();

    const deferred1 = {
      input: Promise.withResolvers<string>(),
      out: Promise.withResolvers<void>(),
    };
    deferredQueue.push(deferred1);
    const deferred2 = {
      input: Promise.withResolvers<string>(),
      out: Promise.withResolvers<void>(),
    };
    deferredQueue.push(deferred2);
    const deferred3 = {
      input: Promise.withResolvers<string>(),
      out: Promise.withResolvers<void>(),
    };
    deferredQueue.push(deferred3);
    deferred1.input.resolve("#");
    deferred2.input.resolve("$");
    await w.write(encoder.encode(`GET / HTTP/1.1\nConnection: keep-alive\n\n`));
    await w.write(encoder.encode(`GET / HTTP/1.1\nConnection: keep-alive\n\n`));

    // Fully read two responses
    let text = "";
    while (!text.includes("$\r\n")) {
      text += decoder.decode((await r.read()).value);
    }

    await w.write(encoder.encode(`GET / HTTP/1.1\nConnection: keep-alive\n\n`));
    await deferred3.out.promise;

    // This is half served, so wait for the chunk that has the first '.'
    text = "";
    while (!text.includes("1\r\n.\r\n")) {
      text += decoder.decode((await r.read()).value);
    }

    abort();

    // This doesn't actually write anything, but we release it after aborting
    deferred3.input.resolve("!");

    // Guarantee: can't connect to an aborted server (though this may not happen immediately)
    let failed = false;
    for (let i = 0; i < 10; i++) {
      try {
        const conn = await Deno.connect({ port: servePort });
        conn.close();
        // Give the runtime a few ticks to settle (required for Windows)
        await new Promise((r) => setTimeout(r, 2 ** i));
        continue;
      } catch (_) {
        failed = true;
        break;
      }
    }
    assert(failed, "The Deno.serve listener was not disabled promptly");

    // Guarantee: the pipeline is closed abruptly
    assert((await r.read()).done);

    try {
      conn.close();
    } catch (_) {
      // Ignore
    }
    await finished;
  },
);

// When shutting down abruptly, we require that all in-progress connections are aborted,
// no new connections are allowed, and no new transactions are allowed on existing connections.
Deno.test(
  { permissions: { net: true } },
  async function httpServerShutdownGracefulGuaranteeHttp11() {
    const deferredQueue: {
      input: ReturnType<typeof Promise.withResolvers<string>>;
      out: ReturnType<typeof Promise.withResolvers<void>>;
    }[] = [];
    const { finished, shutdown } = await makeServer((_req) => {
      const { input, out } = deferredQueue.shift()!;
      return new Response(
        new ReadableStream({
          async start(controller) {
            controller.enqueue(new Uint8Array([46]));
            out.resolve();
            controller.enqueue(encoder.encode(await input.promise));
            controller.close();
          },
        }),
      );
    });
    const encoder = new TextEncoder();
    const decoder = new TextDecoder();
    const conn = await Deno.connect({ port: servePort });
    const w = conn.writable.getWriter();
    const r = conn.readable.getReader();

    const deferred1 = {
      input: Promise.withResolvers<string>(),
      out: Promise.withResolvers<void>(),
    };
    deferredQueue.push(deferred1);
    const deferred2 = {
      input: Promise.withResolvers<string>(),
      out: Promise.withResolvers<void>(),
    };
    deferredQueue.push(deferred2);
    const deferred3 = {
      input: Promise.withResolvers<string>(),
      out: Promise.withResolvers<void>(),
    };
    deferredQueue.push(deferred3);
    deferred1.input.resolve("#");
    deferred2.input.resolve("$");
    await w.write(encoder.encode(`GET / HTTP/1.1\nConnection: keep-alive\n\n`));
    await w.write(encoder.encode(`GET / HTTP/1.1\nConnection: keep-alive\n\n`));

    // Fully read two responses
    let text = "";
    while (!text.includes("$\r\n")) {
      text += decoder.decode((await r.read()).value);
    }

    await w.write(encoder.encode(`GET / HTTP/1.1\nConnection: keep-alive\n\n`));
    await deferred3.out.promise;

    // This is half served, so wait for the chunk that has the first '.'
    text = "";
    while (!text.includes("1\r\n.\r\n")) {
      text += decoder.decode((await r.read()).value);
    }

    const shutdownPromise = shutdown();

    // Release the final response _after_ we shut down
    deferred3.input.resolve("!");

    // Guarantee: can't connect to an aborted server (though this may not happen immediately)
    let failed = false;
    for (let i = 0; i < 10; i++) {
      try {
        const conn = await Deno.connect({ port: servePort });
        conn.close();
        // Give the runtime a few ticks to settle (required for Windows)
        await new Promise((r) => setTimeout(r, 2 ** i));
        continue;
      } catch (_) {
        failed = true;
        break;
      }
    }
    assert(failed, "The Deno.serve listener was not disabled promptly");

    // Guarantee: existing connections fully drain
    while (!text.includes("!\r\n")) {
      text += decoder.decode((await r.read()).value);
    }

    await shutdownPromise;

    try {
      conn.close();
    } catch (_) {
      // Ignore
    }
    await finished;
  },
);

// Ensure that resources don't leak during a graceful shutdown
Deno.test(
  { permissions: { net: true, write: true, read: true } },
  async function httpServerShutdownGracefulResources() {
    const { promise, resolve } = Promise.withResolvers<void>();
    const { finished, shutdown } = await makeServer(async (_req) => {
      resolve();
      await new Promise((r) => setTimeout(r, 10));
      return new Response((await makeTempFile(1024 * 1024)).readable);
    });

    const f = fetch(`http://localhost:${servePort}`);
    await promise;
    assertEquals((await (await f).text()).length, 1048576);
    await shutdown();
    await finished;
  },
);

// Ensure that resources don't leak during a graceful shutdown
Deno.test(
  { permissions: { net: true, write: true, read: true } },
  async function httpServerShutdownGracefulResources2() {
    const waitForAbort = Promise.withResolvers<void>();
    const waitForRequest = Promise.withResolvers<void>();
    const { finished, shutdown } = await makeServer(async (_req) => {
      waitForRequest.resolve();
      await waitForAbort.promise;
      await new Promise((r) => setTimeout(r, 10));
      return new Response((await makeTempFile(1024 * 1024)).readable);
    });

    const f = fetch(`http://localhost:${servePort}`);
    await waitForRequest.promise;
    const s = shutdown();
    waitForAbort.resolve();
    assertEquals((await (await f).text()).length, 1048576);
    await s;
    await finished;
  },
);

Deno.test(
  { permissions: { net: true, write: true, read: true } },
  async function httpServerExplicitResourceManagement() {
    let dataPromise;

    {
      await using _server = await makeServer(async (_req) => {
        return new Response((await makeTempFile(1024 * 1024)).readable);
      });

      const resp = await fetch(`http://localhost:${servePort}`);
      dataPromise = resp.bytes();
    }

    assertEquals((await dataPromise).byteLength, 1048576);
  },
);

Deno.test(
  { permissions: { net: true, write: true, read: true } },
  async function httpServerExplicitResourceManagementManualClose() {
    await using server = await makeServer(async (_req) => {
      return new Response((await makeTempFile(1024 * 1024)).readable);
    });

    const resp = await fetch(`http://localhost:${servePort}`);

    const [_, data] = await Promise.all([
      server.shutdown(),
      resp.bytes(),
    ]);

    assertEquals(data.byteLength, 1048576);
  },
);

Deno.test(
  { permissions: { read: true, run: true } },
  async function httpServerUnref() {
    const [statusCode, _output] = await execCode(`
      async function main() {
        const server = Deno.serve({ port: ${servePort}, handler: () => null });
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
  const { promise, resolve } = Promise.withResolvers<void>();

  const server = Deno.serve({
    handler: (_req) => new Response("ok"),
    hostname: "localhost",
    port: servePort,
    signal: ac.signal,
    onListen: onListen(resolve),
    onError: createOnErrorCb(ac),
  });

  await promise;
  const resp = await fetch(`http://localhost:${servePort}/`, {
    headers: { "connection": "close" },
  });
  const text = await resp.text();
  assertEquals(text, "ok");
  ac.abort();
  await server.finished;
});

Deno.test(async function httpServerRejectsOnAddrInUse() {
  const ac = new AbortController();
  const { promise, resolve } = Promise.withResolvers<void>();

  const server = Deno.serve({
    handler: (_req) => new Response("ok"),
    hostname: "localhost",
    port: servePort,
    signal: ac.signal,
    onListen: onListen(resolve),
    onError: createOnErrorCb(ac),
  });
  await promise;

  assertThrows(
    () =>
      Deno.serve({
        handler: (_req) => new Response("ok"),
        hostname: "localhost",
        port: servePort,
        signal: ac.signal,
        onListen: onListen(resolve),
        onError: createOnErrorCb(ac),
      }),
    Deno.errors.AddrInUse,
  );
  ac.abort();
  await server.finished;
});

Deno.test({ permissions: { net: true } }, async function httpServerBasic() {
  const ac = new AbortController();
  const deferred = Promise.withResolvers<void>();
  const listeningDeferred = Promise.withResolvers<Deno.NetAddr>();

  const server = Deno.serve({
    handler: async (request, { remoteAddr }) => {
      // FIXME(bartlomieju):
      // make sure that request can be inspected
      console.log(request);
      assertEquals(new URL(request.url).href, `http://127.0.0.1:${servePort}/`);
      assertEquals(await request.text(), "");
      assertEquals(remoteAddr.hostname, "127.0.0.1");
      deferred.resolve();
      return new Response("Hello World", { headers: { "foo": "bar" } });
    },
    port: servePort,
    signal: ac.signal,
    onListen: (addr) => listeningDeferred.resolve(addr),
    onError: createOnErrorCb(ac),
  });

  const addr = await listeningDeferred.promise;
  assertEquals(addr.hostname, server.addr.hostname);
  assertEquals(addr.port, server.addr.port);
  const resp = await fetch(`http://127.0.0.1:${servePort}/`, {
    headers: { "connection": "close" },
  });
  await deferred.promise;
  const clone = resp.clone();
  const text = await resp.text();
  assertEquals(text, "Hello World");
  assertEquals(resp.headers.get("foo"), "bar");
  const cloneText = await clone.text();
  assertEquals(cloneText, "Hello World");
  ac.abort();
  await server.finished;
});

// Test serving of HTTP on an arbitrary listener.
Deno.test(
  { permissions: { net: true } },
  async function httpServerOnListener() {
    const ac = new AbortController();
    const deferred = Promise.withResolvers<void>();
    const listeningDeferred = Promise.withResolvers();
    const listener = Deno.listen({ port: servePort });
    const server = serveHttpOnListener(
      listener,
      ac.signal,
      async (
        request: Request,
        { remoteAddr }: { remoteAddr: { hostname: string } },
      ) => {
        assertEquals(
          new URL(request.url).href,
          `http://127.0.0.1:${servePort}/`,
        );
        assertEquals(await request.text(), "");
        assertEquals(remoteAddr.hostname, "127.0.0.1");
        deferred.resolve();
        return new Response("Hello World", { headers: { "foo": "bar" } });
      },
      createOnErrorCb(ac),
      onListen(listeningDeferred.resolve),
    );

    await listeningDeferred.promise;
    const resp = await fetch(`http://127.0.0.1:${servePort}/`, {
      headers: { "connection": "close" },
    });
    await listeningDeferred.promise;
    const clone = resp.clone();
    const text = await resp.text();
    assertEquals(text, "Hello World");
    assertEquals(resp.headers.get("foo"), "bar");
    const cloneText = await clone.text();
    assertEquals(cloneText, "Hello World");
    ac.abort();
    await server.finished;
  },
);

// Test serving of HTTP on an arbitrary connection.
Deno.test(
  { permissions: { net: true } },
  async function httpServerOnConnection() {
    const ac = new AbortController();
    const deferred = Promise.withResolvers<void>();
    const listeningDeferred = Promise.withResolvers<void>();
    const listener = Deno.listen({ port: servePort });
    const acceptPromise = listener.accept();
    const fetchPromise = fetch(`http://127.0.0.1:${servePort}/`, {
      headers: { "connection": "close" },
    });

    const server = serveHttpOnConnection(
      await acceptPromise,
      ac.signal,
      async (
        request: Request,
        { remoteAddr }: { remoteAddr: { hostname: string } },
      ) => {
        assertEquals(
          new URL(request.url).href,
          `http://127.0.0.1:${servePort}/`,
        );
        assertEquals(await request.text(), "");
        assertEquals(remoteAddr.hostname, "127.0.0.1");
        deferred.resolve();
        return new Response("Hello World", { headers: { "foo": "bar" } });
      },
      createOnErrorCb(ac),
      onListen(listeningDeferred.resolve),
    );

    const resp = await fetchPromise;
    await deferred.promise;
    const clone = resp.clone();
    const text = await resp.text();
    assertEquals(text, "Hello World");
    assertEquals(resp.headers.get("foo"), "bar");
    const cloneText = await clone.text();
    assertEquals(cloneText, "Hello World");
    // Note that we don't need to abort this server -- it closes when the connection does
    // ac.abort();
    await server.finished;
    listener.close();
  },
);

Deno.test({ permissions: { net: true } }, async function httpServerOnError() {
  const ac = new AbortController();
  const { promise, resolve } = Promise.withResolvers<void>();
  let requestStash: Request | null;

  const server = Deno.serve({
    handler: async (request: Request) => {
      requestStash = request;
      await new Promise((r) => setTimeout(r, 100));
      throw "fail";
    },
    port: servePort,
    signal: ac.signal,
    onListen: onListen(resolve),
    onError: () => {
      return new Response("failed: " + requestStash!.url, { status: 500 });
    },
  });

  await promise;
  const resp = await fetch(`http://127.0.0.1:${servePort}/`, {
    headers: { "connection": "close" },
  });
  const text = await resp.text();
  ac.abort();
  await server.finished;

  assertEquals(text, `failed: http://127.0.0.1:${servePort}/`);
});

Deno.test(
  { permissions: { net: true } },
  async function httpServerOnErrorFails() {
    const ac = new AbortController();
    const { promise, resolve } = Promise.withResolvers<void>();
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
      onListen: onListen(resolve),
      onError: () => {
        throw "again";
      },
    });

    await promise;
    const resp = await fetch(`http://127.0.0.1:${servePort}/`, {
      headers: { "connection": "close" },
    });
    const text = await resp.text();
    ac.abort();
    await server.finished;

    assertEquals(text, "Internal Server Error");
  },
);

Deno.test({ permissions: { net: true } }, async function httpServerOverload1() {
  const ac = new AbortController();
  const deferred = Promise.withResolvers<void>();
  const listeningDeferred = Promise.withResolvers<void>();

  const server = Deno.serve({
    port: servePort,
    signal: ac.signal,
    onListen: onListen(listeningDeferred.resolve),
    onError: createOnErrorCb(ac),
  }, async (request) => {
    // FIXME(bartlomieju):
    // make sure that request can be inspected
    console.log(request);
    assertEquals(new URL(request.url).href, `http://127.0.0.1:${servePort}/`);
    assertEquals(await request.text(), "");
    deferred.resolve();
    return new Response("Hello World", { headers: { "foo": "bar" } });
  });

  await listeningDeferred.promise;
  const resp = await fetch(`http://127.0.0.1:${servePort}/`, {
    headers: { "connection": "close" },
  });
  await deferred.promise;
  const clone = resp.clone();
  const text = await resp.text();
  assertEquals(text, "Hello World");
  assertEquals(resp.headers.get("foo"), "bar");
  const cloneText = await clone.text();
  assertEquals(cloneText, "Hello World");
  ac.abort();
  await server.finished;
});

Deno.test({ permissions: { net: true } }, async function httpServerOverload2() {
  const ac = new AbortController();
  const deferred = Promise.withResolvers<void>();
  const listeningDeferred = Promise.withResolvers<void>();

  const server = Deno.serve({
    port: servePort,
    signal: ac.signal,
    onListen: onListen(listeningDeferred.resolve),
    onError: createOnErrorCb(ac),
  }, async (request) => {
    // FIXME(bartlomieju):
    // make sure that request can be inspected
    console.log(request);
    assertEquals(new URL(request.url).href, `http://127.0.0.1:${servePort}/`);
    assertEquals(await request.text(), "");
    deferred.resolve();
    return new Response("Hello World", { headers: { "foo": "bar" } });
  });

  await listeningDeferred.promise;
  const resp = await fetch(`http://127.0.0.1:${servePort}/`, {
    headers: { "connection": "close" },
  });
  await deferred.promise;
  const clone = resp.clone();
  const text = await resp.text();
  assertEquals(text, "Hello World");
  assertEquals(resp.headers.get("foo"), "bar");
  const cloneText = await clone.text();
  assertEquals(cloneText, "Hello World");
  ac.abort();
  await server.finished;
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
  await server.finished;
});

Deno.test(
  { permissions: { net: true }, ignore: Deno.build.os !== "windows" },
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

      await server.finished;
    } finally {
      console.log = consoleLog;
    }
  },
);

// https://github.com/denoland/deno/issues/15107
Deno.test(
  { permissions: { net: true } },
  async function httpLazyHeadersIssue15107() {
    const deferred = Promise.withResolvers<void>();
    const listeningDeferred = Promise.withResolvers<void>();
    const ac = new AbortController();

    let headers: Headers;
    const server = Deno.serve({
      handler: async (request) => {
        await request.text();
        headers = request.headers;
        deferred.resolve();
        return new Response("");
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningDeferred.resolve),
      onError: createOnErrorCb(ac),
    });

    await listeningDeferred.promise;
    const conn = await Deno.connect({ port: servePort });
    // Send GET request with a body + content-length.
    const encoder = new TextEncoder();
    const body =
      `GET / HTTP/1.1\r\nHost: 127.0.0.1:2333\r\nContent-Length: 5\r\n\r\n12345`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);
    await deferred.promise;
    conn.close();
    assertEquals(headers!.get("content-length"), "5");
    ac.abort();
    await server.finished;
  },
);

function createUrlTest(
  name: string,
  methodAndPath: string,
  host: string | null,
  expected: string,
) {
  Deno.test(`httpServerUrl${name}`, async () => {
    const listeningDeferred = Promise.withResolvers<number>();
    const urlDeferred = Promise.withResolvers<string>();
    const ac = new AbortController();
    const server = Deno.serve({
      handler: (request: Request) => {
        urlDeferred.resolve(request.url);
        return new Response("");
      },
      port: 0,
      signal: ac.signal,
      onListen: ({ port }: { port: number }) => {
        listeningDeferred.resolve(port);
      },
      onError: createOnErrorCb(ac),
    });

    const port = await listeningDeferred.promise;
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
      assertEquals(await urlDeferred.promise, expectedResult);
    } finally {
      ac.abort();
      await server.finished;
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
    const deferred = Promise.withResolvers<void>();
    const ac = new AbortController();
    const listeningDeferred = Promise.withResolvers<void>();

    const server = Deno.serve({
      handler: (request) => {
        assertEquals(request.body, null);
        deferred.resolve();
        return new Response("", { headers: {} });
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningDeferred.resolve),
      onError: createOnErrorCb(ac),
    });

    await listeningDeferred.promise;
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
    await deferred.promise;
    ac.abort();
    await server.finished;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerAbortedRequestBody() {
    const deferred = Promise.withResolvers<void>();
    const ac = new AbortController();
    const listeningDeferred = Promise.withResolvers<void>();

    const server = Deno.serve({
      handler: async (request) => {
        await assertRejects(async () => {
          await request.text();
        });
        deferred.resolve();
        // Not actually used
        return new Response();
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningDeferred.resolve),
      onError: createOnErrorCb(ac),
    });

    await listeningDeferred.promise;
    const conn = await Deno.connect({ port: servePort });
    // Send POST request with a body + content-length, but don't send it all
    const encoder = new TextEncoder();
    const body =
      `POST / HTTP/1.1\r\nHost: 127.0.0.1:${servePort}\r\nContent-Length: 10\r\n\r\n12345`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);
    conn.close();
    await deferred.promise;
    ac.abort();
    await server.finished;
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
    const { promise, resolve } = Promise.withResolvers<void>();
    const server = Deno.serve({
      handler: (_request) => {
        return new Response(makeStream(count, delay));
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(resolve),
      onError: createOnErrorCb(ac),
    });

    try {
      await promise;
      const resp = await fetch(`http://127.0.0.1:${servePort}/`);
      if (action == "Throw") {
        await assertRejects(async () => {
          await resp.text();
        });
      } else {
        const text = await resp.text();

        let expected = "";
        for (let i = 0; i < count; i++) {
          expected += `a${i}`;
        }

        assertEquals(text, expected);
      }
    } finally {
      ac.abort();
      await server.shutdown();
    }
  });
}

for (const count of [0, 1, 2, 3]) {
  for (const delay of [0, 1, 25]) {
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
    const { promise, resolve } = Promise.withResolvers<void>();
    const ac = new AbortController();
    const server = Deno.serve({
      handler: async (request) => {
        const reqBody = await request.text();
        assertEquals("hello world", reqBody);
        return new Response("yo");
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(resolve),
      onError: createOnErrorCb(ac),
    });

    await promise;
    const resp = await fetch(`http://127.0.0.1:${servePort}/`, {
      body: stream.readable,
      method: "POST",
      headers: { "connection": "close" },
    });

    assertEquals(await resp.text(), "yo");
    ac.abort();
    await server.finished;
  },
);

Deno.test({ permissions: { net: true } }, async function httpServerClose() {
  const ac = new AbortController();
  const { promise, resolve } = Promise.withResolvers<void>();
  const server = Deno.serve({
    handler: () => new Response("ok"),
    port: servePort,
    signal: ac.signal,
    onListen: onListen(resolve),
    onError: createOnErrorCb(ac),
  });
  await promise;
  const client = await Deno.connect({ port: servePort });
  client.close();
  ac.abort();
  await server.finished;
});

// https://github.com/denoland/deno/issues/15427
Deno.test({ permissions: { net: true } }, async function httpServerCloseGet() {
  const ac = new AbortController();
  const listeningDeferred = Promise.withResolvers<void>();
  const requestDeferred = Promise.withResolvers<void>();
  const responseDeferred = Promise.withResolvers<void>();
  const server = Deno.serve({
    handler: async () => {
      requestDeferred.resolve();
      await new Promise((r) => setTimeout(r, 500));
      responseDeferred.resolve();
      return new Response("ok");
    },
    port: servePort,
    signal: ac.signal,
    onListen: onListen(listeningDeferred.resolve),
    onError: createOnErrorCb(ac),
  });
  await listeningDeferred.promise;
  const conn = await Deno.connect({ port: servePort });
  const encoder = new TextEncoder();
  const body =
    `GET / HTTP/1.1\r\nHost: example.domain\r\nConnection: close\r\n\r\n`;
  const writeResult = await conn.write(encoder.encode(body));
  assertEquals(body.length, writeResult);
  await requestDeferred.promise;
  conn.close();
  await responseDeferred.promise;
  ac.abort();
  await server.finished;
});

// FIXME:
Deno.test(
  { permissions: { net: true } },
  async function httpServerEmptyBlobResponse() {
    const ac = new AbortController();
    const { promise, resolve } = Promise.withResolvers<void>();
    const server = Deno.serve({
      handler: () => new Response(new Blob([])),
      port: servePort,
      signal: ac.signal,
      onListen: onListen(resolve),
      onError: createOnErrorCb(ac),
    });

    await promise;
    const resp = await fetch(`http://127.0.0.1:${servePort}/`);
    const respBody = await resp.text();

    assertEquals("", respBody);
    ac.abort();
    await server.finished;
  },
);

// https://github.com/denoland/deno/issues/17291
Deno.test(
  { permissions: { net: true } },
  async function httpServerIncorrectChunkedResponse() {
    const ac = new AbortController();
    const listeningDeferred = Promise.withResolvers<void>();
    const errorDeferred = Promise.withResolvers<void>();
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
      onListen: onListen(listeningDeferred.resolve),
      onError: (err) => {
        const errResp = new Response(
          `Internal server error: ${(err as Error).message}`,
          { status: 500 },
        );
        errorDeferred.resolve();
        return errResp;
      },
    });

    await listeningDeferred.promise;
    const resp = await fetch(`http://127.0.0.1:${servePort}/`);
    // Incorrectly implemented reader ReadableStream should reject.
    assertStringIncludes(await resp.text(), "Failed to execute 'enqueue'");
    await errorDeferred.promise;
    ac.abort();
    await server.finished;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerCorrectLengthForUnicodeString() {
    const ac = new AbortController();
    const { promise, resolve } = Promise.withResolvers<void>();

    const server = Deno.serve({
      handler: () => new Response("韓國".repeat(10)),
      port: servePort,
      signal: ac.signal,
      onListen: onListen(resolve),
      onError: createOnErrorCb(ac),
    });

    await promise;
    const conn = await Deno.connect({ port: servePort });
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
    await server.finished;
    assert(msg.includes("content-length: 60"));
  },
);

Deno.test({ permissions: { net: true } }, async function httpServerWebSocket() {
  const ac = new AbortController();
  const listeningDeferred = Promise.withResolvers<void>();
  const doneDeferred = Promise.withResolvers<void>();
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
      socket.onclose = () => doneDeferred.resolve();
      return response;
    },
    port: servePort,
    signal: ac.signal,
    onListen: onListen(listeningDeferred.resolve),
    onError: createOnErrorCb(ac),
  });

  await listeningDeferred.promise;
  const def = Promise.withResolvers<void>();
  const ws = new WebSocket(`ws://localhost:${servePort}`);
  ws.onmessage = (m) => assertEquals(m.data, "foo");
  ws.onerror = (e) => {
    console.error(e);
    fail();
  };
  ws.onclose = () => def.resolve();
  ws.onopen = () => ws.send("foo");

  await def.promise;
  await doneDeferred.promise;
  ac.abort();
  await server.finished;
});

Deno.test(
  { permissions: { net: true } },
  async function httpServerWebSocketRaw() {
    const ac = new AbortController();
    const { promise, resolve } = Promise.withResolvers<void>();
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
      onListen: onListen(resolve),
      onError: createOnErrorCb(ac),
    });

    await promise;

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
    await server.finished;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerWebSocketUpgradeTwice() {
    const ac = new AbortController();
    const done = Promise.withResolvers<void>();
    const listeningDeferred = Promise.withResolvers<void>();
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
        socket.onclose = () => done.resolve();
        return response;
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningDeferred.resolve),
      onError: createOnErrorCb(ac),
    });

    await listeningDeferred.promise;
    const def = Promise.withResolvers<void>();
    const ws = new WebSocket(`ws://localhost:${servePort}`);
    ws.onmessage = (m) => assertEquals(m.data, "foo");
    ws.onerror = (e) => {
      console.error(e);
      fail();
    };
    ws.onclose = () => def.resolve();
    ws.onopen = () => ws.send("foo");

    await def.promise;
    await done.promise;
    ac.abort();
    await server.finished;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerWebSocketCloseFast() {
    const ac = new AbortController();
    const done = Promise.withResolvers<void>();
    const listeningDeferred = Promise.withResolvers<void>();
    const server = Deno.serve({
      handler: (request) => {
        const {
          response,
          socket,
        } = Deno.upgradeWebSocket(request);
        socket.onopen = () => socket.close();
        socket.onclose = () => done.resolve();
        return response;
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningDeferred.resolve),
      onError: createOnErrorCb(ac),
    });

    await listeningDeferred.promise;
    const def = Promise.withResolvers<void>();
    const ws = new WebSocket(`ws://localhost:${servePort}`);
    ws.onerror = (e) => {
      console.error(e);
      fail();
    };
    ws.onclose = () => def.resolve();

    await def.promise;
    await done.promise;
    ac.abort();
    await server.finished;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerWebSocketCanAccessRequest() {
    const ac = new AbortController();
    const done = Promise.withResolvers<void>();
    const listeningDeferred = Promise.withResolvers<void>();
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
        socket.onclose = () => done.resolve();
        return response;
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningDeferred.resolve),
      onError: createOnErrorCb(ac),
    });

    await listeningDeferred.promise;
    const def = Promise.withResolvers<void>();
    const ws = new WebSocket(`ws://localhost:${servePort}`);
    ws.onmessage = (m) =>
      assertEquals(m.data, `http://localhost:${servePort}/`);
    ws.onerror = (e) => {
      console.error(e);
      fail();
    };
    ws.onclose = () => def.resolve();
    ws.onopen = () => ws.send("foo");

    await def.promise;
    await done.promise;
    ac.abort();
    await server.finished;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpVeryLargeRequest() {
    const deferred = Promise.withResolvers<void>();
    const listeningDeferred = Promise.withResolvers<void>();
    const ac = new AbortController();

    let headers: Headers;
    const server = Deno.serve({
      handler: (request) => {
        headers = request.headers;
        deferred.resolve();
        return new Response("");
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningDeferred.resolve),
      onError: createOnErrorCb(ac),
    });

    await listeningDeferred.promise;
    const conn = await Deno.connect({ port: servePort });
    // Send GET request with a body + content-length.
    const encoder = new TextEncoder();
    const smthElse = "x".repeat(16 * 1024 + 256);
    const body =
      `GET / HTTP/1.1\r\nHost: 127.0.0.1:2333\r\nContent-Length: 5\r\nSomething-Else: ${smthElse}\r\n\r\n`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);
    await deferred.promise;
    conn.close();
    assertEquals(headers!.get("content-length"), "5");
    assertEquals(headers!.get("something-else"), smthElse);
    ac.abort();
    await server.finished;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpVeryLargeRequestAndBody() {
    const deferred = Promise.withResolvers<void>();
    const listeningDeferred = Promise.withResolvers<void>();
    const ac = new AbortController();

    let headers: Headers;
    let text: string;
    const server = Deno.serve({
      handler: async (request) => {
        headers = request.headers;
        text = await request.text();
        deferred.resolve();
        return new Response("");
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningDeferred.resolve),
      onError: createOnErrorCb(ac),
    });

    await listeningDeferred.promise;
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

    await deferred.promise;
    conn.close();

    assertEquals(headers!.get("content-length"), `${reqBody.length}`);
    assertEquals(headers!.get("something-else"), smthElse);
    assertEquals(text!, reqBody);
    ac.abort();
    await server.finished;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpConnectionClose() {
    const deferred = Promise.withResolvers<void>();
    const ac = new AbortController();
    const listeningDeferred = Promise.withResolvers<void>();

    const server = Deno.serve({
      handler: () => {
        deferred.resolve();
        return new Response("");
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningDeferred.resolve),
      onError: createOnErrorCb(ac),
    });

    await listeningDeferred.promise;
    const conn = await Deno.connect({ port: servePort });
    // Send GET request with a body + connection: close.
    const encoder = new TextEncoder();
    const body =
      `GET / HTTP/1.1\r\nHost: 127.0.0.1:2333\r\nConnection: Close\r\n\r\n`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);

    await deferred.promise;
    conn.close();

    ac.abort();
    await server.finished;
  },
);

async function testDuplex(
  reader: ReadableStreamDefaultReader<Uint8Array>,
  writable: WritableStreamDefaultWriter<Uint8Array>,
) {
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
}

Deno.test(
  { permissions: { net: true } },
  async function httpServerStreamDuplexDirect() {
    const { promise, resolve } = Promise.withResolvers<void>();
    const ac = new AbortController();

    const server = Deno.serve(
      { port: servePort, signal: ac.signal },
      (request: Request) => {
        assert(request.body);
        resolve();
        return new Response(request.body);
      },
    );

    const { readable, writable } = new TransformStream();
    const resp = await fetch(`http://127.0.0.1:${servePort}/`, {
      method: "POST",
      body: readable,
    });

    await promise;
    assert(resp.body);
    await testDuplex(resp.body.getReader(), writable.getWriter());
    ac.abort();
    await server.finished;
  },
);

// Test that a duplex stream passing through JavaScript also works (ie: that the request body resource
// is still alive). https://github.com/denoland/deno/pull/20206
Deno.test(
  { permissions: { net: true } },
  async function httpServerStreamDuplexJavascript() {
    const { promise, resolve } = Promise.withResolvers<void>();
    const ac = new AbortController();

    const server = Deno.serve(
      { port: servePort, signal: ac.signal },
      (request: Request) => {
        assert(request.body);
        resolve();
        const reader = request.body.getReader();
        return new Response(
          new ReadableStream({
            async pull(controller) {
              await new Promise((r) => setTimeout(r, 100));
              const { done, value } = await reader.read();
              if (done) {
                controller.close();
              } else {
                controller.enqueue(value);
              }
            },
          }),
        );
      },
    );

    const { readable, writable } = new TransformStream();
    const resp = await fetch(`http://127.0.0.1:${servePort}/`, {
      method: "POST",
      body: readable,
    });

    await promise;
    assert(resp.body);
    await testDuplex(resp.body.getReader(), writable.getWriter());
    ac.abort();
    await server.finished;
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
    const listeningDeferred = Promise.withResolvers<void>();
    const deferred = Promise.withResolvers<void>();
    const ac = new AbortController();

    let counter = 0;

    const deferreds = [
      Promise.withResolvers<void>(),
      Promise.withResolvers<void>(),
      Promise.withResolvers<void>(),
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

          await deferreds[counter - 1].promise;

          controller.enqueue(`${counter}\n`);
          counter++;
        },
      }).pipeThrough(new TextEncoderStream());
    }

    const server = Deno.serve({
      handler: () => {
        deferred.resolve();
        return new Response(periodicStream());
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningDeferred.resolve),
      onError: createOnErrorCb(ac),
    });

    await listeningDeferred.promise;
    // start a client
    const clientConn = await Deno.connect({ port: servePort });

    const r1 = await writeRequest(clientConn);
    assertEquals(r1, "0\n1\n2\n");

    ac.abort();
    await deferred.promise;
    await server.finished;
    clientConn.close();
  },
);

// Make sure that the chunks of a large response aren't repeated or corrupted in some other way by
// scatterning sentinels throughout.
// https://github.com/denoland/fresh/issues/1699
Deno.test(
  { permissions: { net: true } },
  async function httpLargeReadableStreamChunk() {
    const ac = new AbortController();
    const server = Deno.serve({
      handler() {
        return new Response(
          new ReadableStream({
            start(controller) {
              const buffer = new Uint8Array(1024 * 1024);
              // Mark the buffer with sentinels
              for (let i = 0; i < 256; i++) {
                buffer[i * 4096] = i;
              }
              controller.enqueue(buffer);
              controller.close();
            },
          }),
        );
      },
      port: servePort,
      signal: ac.signal,
    });
    const response = await fetch(`http://localhost:${servePort}/`);
    const body = await response.bytes();
    assertEquals(1024 * 1024, body.byteLength);
    for (let i = 0; i < 256; i++) {
      assertEquals(
        i,
        body[i * 4096],
        `sentinel mismatch at index ${i * 4096}`,
      );
    }
    ac.abort();
    await server.finished;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpRequestLatin1Headers() {
    const listeningDeferred = Promise.withResolvers<void>();
    const deferred = Promise.withResolvers<void>();
    const ac = new AbortController();
    const server = Deno.serve({
      handler: (request) => {
        assertEquals(request.headers.get("X-Header-Test"), "á");
        deferred.resolve();
        return new Response("hello", { headers: { "X-Header-Test": "Æ" } });
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningDeferred.resolve),
      onError: createOnErrorCb(ac),
    });

    await listeningDeferred.promise;
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

    await deferred.promise;
    const responseText = new TextDecoder("iso-8859-1").decode(buf);
    clientConn.close();

    ac.abort();
    await server.finished;

    assertMatch(responseText, /\r\n[Xx]-[Hh]eader-[Tt]est: Æ\r\n/);
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerRequestWithoutPath() {
    const deferred = Promise.withResolvers<void>();
    const listeningDeferred = Promise.withResolvers<void>();
    const ac = new AbortController();

    const server = Deno.serve({
      handler: async (request) => {
        // FIXME:
        // assertEquals(new URL(request.url).href, `http://127.0.0.1:${servePort}/`);
        assertEquals(await request.text(), "");
        deferred.resolve();
        return new Response("11");
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningDeferred.resolve),
      onError: createOnErrorCb(ac),
    });

    await listeningDeferred.promise;
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
    await deferred.promise;

    ac.abort();
    await server.finished;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpCookieConcatenation() {
    const deferred = Promise.withResolvers<void>();
    const listeningDeferred = Promise.withResolvers<void>();
    const ac = new AbortController();

    const server = Deno.serve({
      handler: async (request) => {
        assertEquals(await request.text(), "");
        assertEquals(request.headers.get("cookie"), "foo=bar; bar=foo");
        deferred.resolve();
        return new Response("ok");
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningDeferred.resolve),
      onError: createOnErrorCb(ac),
      reusePort: true,
    });

    await listeningDeferred.promise;
    const resp = await fetch(`http://127.0.0.1:${servePort}/`, {
      headers: [
        ["connection", "close"],
        ["cookie", "foo=bar"],
        ["cookie", "bar=foo"],
      ],
    });
    await deferred.promise;

    const text = await resp.text();
    assertEquals(text, "ok");

    ac.abort();
    await server.finished;
  },
);

// https://github.com/denoland/deno/issues/12741
// https://github.com/denoland/deno/pull/12746
// https://github.com/denoland/deno/pull/12798
Deno.test(
  { permissions: { net: true, run: true } },
  async function httpServerDeleteRequestHasBody() {
    const deferred = Promise.withResolvers<void>();
    const listeningDeferred = Promise.withResolvers<void>();
    const ac = new AbortController();

    const hostname = "localhost";

    const server = Deno.serve({
      handler: () => {
        deferred.resolve();
        return new Response("ok");
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningDeferred.resolve),
      onError: createOnErrorCb(ac),
    });

    await listeningDeferred.promise;
    const url = `http://${hostname}:${servePort}/`;
    const args = ["-X", "DELETE", url];
    const { success } = await new Deno.Command("curl", {
      args,
      stdout: "null",
      stderr: "null",
    }).output();
    assert(success);
    await deferred.promise;
    ac.abort();

    await server.finished;
  },
);

// FIXME:
Deno.test(
  { permissions: { net: true } },
  async function httpServerRespondNonAsciiUint8Array() {
    const deferred = Promise.withResolvers<void>();
    const listeningDeferred = Promise.withResolvers<void>();
    const ac = new AbortController();

    const server = Deno.serve({
      handler: (request) => {
        assertEquals(request.body, null);
        deferred.resolve();
        return new Response(new Uint8Array([128]));
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningDeferred.resolve),
      onError: createOnErrorCb(ac),
    });
    await listeningDeferred.resolve;
    const resp = await fetch(`http://localhost:${servePort}/`);

    await deferred.promise;

    assertEquals(resp.status, 200);
    const body = await resp.bytes();
    assertEquals(body, new Uint8Array([128]));

    ac.abort();
    await server.finished;
  },
);

// Some of these tests are ported from Hyper
// https://github.com/hyperium/hyper/blob/889fa2d87252108eb7668b8bf034ffcc30985117/src/proto/h1/role.rs
// https://github.com/hyperium/hyper/blob/889fa2d87252108eb7668b8bf034ffcc30985117/tests/server.rs

Deno.test(
  { permissions: { net: true } },
  async function httpServerParseRequest() {
    const deferred = Promise.withResolvers<void>();
    const listeningDeferred = Promise.withResolvers<void>();
    const ac = new AbortController();

    const server = Deno.serve({
      handler: (request) => {
        assertEquals(request.method, "GET");
        assertEquals(request.headers.get("host"), "deno.land");
        deferred.resolve();
        return new Response("ok");
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningDeferred.resolve),
      onError: createOnErrorCb(ac),
    });

    await listeningDeferred.promise;
    const conn = await Deno.connect({ port: servePort });
    const encoder = new TextEncoder();
    const body = `GET /echo HTTP/1.1\r\nHost: deno.land\r\n\r\n`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);
    await deferred.promise;
    conn.close();

    ac.abort();
    await server.finished;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerParseHeaderHtabs() {
    const deferred = Promise.withResolvers<void>();
    const listeningDeferred = Promise.withResolvers<void>();
    const ac = new AbortController();

    const server = Deno.serve({
      handler: (request) => {
        assertEquals(request.method, "GET");
        assertEquals(request.headers.get("server"), "hello\tworld");
        deferred.resolve();
        return new Response("ok");
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningDeferred.resolve),
      onError: createOnErrorCb(ac),
    });

    await listeningDeferred.promise;
    const conn = await Deno.connect({ port: servePort });
    const encoder = new TextEncoder();
    const body = `GET / HTTP/1.1\r\nserver: hello\tworld\r\n\r\n`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);
    await deferred.promise;
    conn.close();

    ac.abort();
    await server.finished;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerGetShouldIgnoreBody() {
    const deferred = Promise.withResolvers<void>();
    const listeningDeferred = Promise.withResolvers<void>();
    const ac = new AbortController();

    const server = Deno.serve({
      handler: async (request) => {
        assertEquals(request.method, "GET");
        assertEquals(await request.text(), "");
        deferred.resolve();
        return new Response("ok");
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningDeferred.resolve),
      onError: createOnErrorCb(ac),
    });

    await listeningDeferred.promise;
    const conn = await Deno.connect({ port: servePort });
    const encoder = new TextEncoder();
    // Connection: close = don't try to parse the body as a new request
    const body =
      `GET / HTTP/1.1\r\nHost: example.domain\r\nConnection: close\r\n\r\nI shouldn't be read.\r\n`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);
    await deferred.promise;
    conn.close();

    ac.abort();
    await server.finished;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerPostWithBody() {
    const deferred = Promise.withResolvers<void>();
    const listeningDeferred = Promise.withResolvers<void>();
    const ac = new AbortController();

    const server = Deno.serve({
      handler: async (request) => {
        assertEquals(request.method, "POST");
        assertEquals(await request.text(), "I'm a good request.");
        deferred.resolve();
        return new Response("ok");
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningDeferred.resolve),
      onError: createOnErrorCb(ac),
    });

    await listeningDeferred.promise;
    const conn = await Deno.connect({ port: servePort });
    const encoder = new TextEncoder();
    const body =
      `POST / HTTP/1.1\r\nHost: example.domain\r\nContent-Length: 19\r\n\r\nI'm a good request.`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);
    await deferred.promise;
    conn.close();

    ac.abort();
    await server.finished;
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
    const deferred = Promise.withResolvers<void>();
    const listeningDeferred = Promise.withResolvers<void>();
    const ac = new AbortController();

    const server = Deno.serve({
      handler: (request) => {
        assertEquals(request.method, "GET");
        deferred.resolve();
        return new Response(testCase.body, testCase.headers ?? {});
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningDeferred.resolve),
      onError: createOnErrorCb(ac),
    });

    await listeningDeferred.promise;
    const conn = await Deno.connect({ port: servePort });
    const encoder = new TextEncoder();
    const body =
      `GET / HTTP/1.1\r\nHost: example.domain\r\nConnection: close\r\n\r\n`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);
    await deferred.promise;

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
    await server.finished;
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
    const deferred = Promise.withResolvers<void>();
    const listeningDeferred = Promise.withResolvers<void>();
    const ac = new AbortController();

    const server = Deno.serve({
      handler: async (request) => {
        assertEquals(request.method, "POST");
        assertEquals(request.headers.get("content-length"), "5");
        assertEquals(await request.text(), "hello");
        deferred.resolve();
        return new Response("ok");
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningDeferred.resolve),
      onError: createOnErrorCb(ac),
    });

    await listeningDeferred.promise;
    const conn = await Deno.connect({ port: servePort });
    const encoder = new TextEncoder();

    const body =
      `POST / HTTP/1.1\r\nHost: example.domain\r\nContent-Length: 5\r\n\r\nhello`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);
    await deferred.promise;

    conn.close();

    ac.abort();
    await server.finished;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerPostWithInvalidPrefixContentLength() {
    const ac = new AbortController();
    const { promise, resolve } = Promise.withResolvers<void>();
    const server = Deno.serve({
      handler: () => {
        throw new Error("unreachable");
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(resolve),
      onError: createOnErrorCb(ac),
    });

    await promise;
    const conn = await Deno.connect({ port: servePort });
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
    await server.finished;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerPostWithChunkedBody() {
    const deferred = Promise.withResolvers<void>();
    const listeningDeferred = Promise.withResolvers<void>();
    const ac = new AbortController();

    const server = Deno.serve({
      handler: async (request) => {
        assertEquals(request.method, "POST");
        assertEquals(await request.text(), "qwert");
        deferred.resolve();
        return new Response("ok");
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningDeferred.resolve),
      onError: createOnErrorCb(ac),
    });

    await listeningDeferred.promise;
    const conn = await Deno.connect({ port: servePort });
    const encoder = new TextEncoder();

    const body =
      `POST / HTTP/1.1\r\nHost: example.domain\r\nTransfer-Encoding: chunked\r\n\r\n1\r\nq\r\n2\r\nwe\r\n2\r\nrt\r\n0\r\n\r\n`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);
    await deferred.promise;

    conn.close();

    ac.abort();
    await server.finished;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerPostWithIncompleteBody() {
    const deferred = Promise.withResolvers<void>();
    const listeningDeferred = Promise.withResolvers<void>();
    const ac = new AbortController();

    const server = Deno.serve({
      handler: async (r) => {
        deferred.resolve();
        assertEquals(await r.text(), "12345");
        return new Response("ok");
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningDeferred.resolve),
      onError: createOnErrorCb(ac),
    });

    await listeningDeferred.promise;
    const conn = await Deno.connect({ port: servePort });
    const encoder = new TextEncoder();

    const body =
      `POST / HTTP/1.1\r\nHost: example.domain\r\nContent-Length: 10\r\n\r\n12345`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);

    await deferred.promise;
    conn.close();

    ac.abort();
    await server.finished;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerHeadResponseDoesntSendBody() {
    const deferred = Promise.withResolvers<void>();
    const listeningDeferred = Promise.withResolvers<void>();
    const ac = new AbortController();

    const server = Deno.serve({
      handler: () => {
        deferred.resolve();
        return new Response("NaN".repeat(100));
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningDeferred.resolve),
      onError: createOnErrorCb(ac),
    });

    await listeningDeferred.promise;
    const conn = await Deno.connect({ port: servePort });
    const encoder = new TextEncoder();
    const decoder = new TextDecoder();

    const body =
      `HEAD / HTTP/1.1\r\nHost: example.domain\r\nConnection: close\r\n\r\n`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);

    await deferred.promise;

    const buf = new Uint8Array(1024);
    const readResult = await conn.read(buf);
    assert(readResult);
    const msg = decoder.decode(buf.subarray(0, readResult));

    assert(msg.includes("content-length: 300\r\n"));

    conn.close();

    ac.abort();
    await server.finished;
  },
);

function makeTempData(size: number) {
  return new Uint8Array(size).fill(1);
}

async function makeTempFile(size: number) {
  const tmpFile = await Deno.makeTempFile();
  using file = await Deno.open(tmpFile, { write: true, read: true });
  const data = makeTempData(size);
  await file.write(data);

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
  { name: "Incompressible", length: 1024, in: {}, out: {}, expect: null },
  {
    name: "IncompressibleAcceptGzip",
    length: 1024,
    in: { "Accept-Encoding": "gzip" },
    out: {},
    expect: null,
  },
  {
    name: "IncompressibleType",
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
    name: "CompressibleType3",
    length: 1024,
    in: { "Accept-Encoding": "br" },
    out: { "Content-Type": "text/plain" },
    expect: "br",
  },
  {
    name: "IncompressibleRange",
    length: 1024,
    in: { "Accept-Encoding": "gzip" },
    out: { "Content-Type": "text/plain", "Content-Range": "1" },
    expect: null,
  },
  {
    name: "IncompressibleCE",
    length: 1024,
    in: { "Accept-Encoding": "gzip" },
    out: { "Content-Type": "text/plain", "Content-Encoding": "random" },
    expect: null,
  },
  {
    name: "IncompressibleCC",
    length: 1024,
    in: { "Accept-Encoding": "gzip" },
    out: { "Content-Type": "text/plain", "Cache-Control": "no-transform" },
    expect: null,
  },
  {
    name: "BadHeader",
    length: 1024,
    in: { "Accept-Encoding": "\x81" },
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
        const deferred = Promise.withResolvers<void>();
        const listeningDeferred = Promise.withResolvers<void>();
        const ac = new AbortController();
        const server = Deno.serve({
          handler: async (_request) => {
            const f = await makeTempFile(testCase.length);
            deferred.resolve();
            // deno-lint-ignore no-explicit-any
            const headers = testCase.out as any;
            headers["Content-Length"] = testCase.length.toString();
            return new Response(f.readable, {
              headers: headers as HeadersInit,
            });
          },
          port: servePort,
          signal: ac.signal,
          onListen: onListen(listeningDeferred.resolve),
          onError: createOnErrorCb(ac),
        });
        try {
          await listeningDeferred.promise;
          const resp = await fetch(`http://127.0.0.1:${servePort}/`, {
            headers: testCase.in as HeadersInit,
          });
          await deferred.promise;
          const body = await resp.bytes();
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
          await server.finished;
        }
      },
    }[name],
  );
}

Deno.test(
  { permissions: { net: true, write: true, read: true } },
  async function httpServerPostFile() {
    const deferred = Promise.withResolvers<void>();
    const listeningDeferred = Promise.withResolvers<void>();
    const ac = new AbortController();

    const server = Deno.serve({
      handler: async (request) => {
        assertEquals(
          await request.bytes(),
          makeTempData(70 * 1024),
        );
        deferred.resolve();
        return new Response("ok");
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningDeferred.resolve),
      onError: createOnErrorCb(ac),
    });

    await listeningDeferred.promise;
    const f = await makeTempFile(70 * 1024);
    const response = await fetch(`http://localhost:${servePort}/`, {
      method: "POST",
      body: f.readable,
    });

    await deferred.promise;

    assertEquals(response.status, 200);
    assertEquals(await response.text(), "ok");

    ac.abort();
    await server.finished;
  },
);

for (const delay of ["delay", "nodelay"]) {
  for (const url of ["text", "file", "stream"]) {
    // Ensure that we don't panic when the incoming TCP request was dropped
    // https://github.com/denoland/deno/issues/20315 and that we correctly
    // close/cancel the response
    Deno.test({
      permissions: { read: true, write: true, net: true },
      name: `httpServerTcpCancellation_${url}_${delay}`,
      fn: async function () {
        const ac = new AbortController();
        const streamCancelled = url == "stream"
          ? Promise.withResolvers<void>()
          : undefined;
        const listeningDeferred = Promise.withResolvers<void>();
        const waitForAbort = Promise.withResolvers<void>();
        const waitForRequest = Promise.withResolvers<void>();
        const server = Deno.serve({
          port: servePort,
          signal: ac.signal,
          onListen: onListen(listeningDeferred.resolve),
          handler: async (req: Request) => {
            let respBody = null;
            if (req.url.includes("/text")) {
              respBody = "text";
            } else if (req.url.includes("/file")) {
              respBody = (await makeTempFile(1024)).readable;
            } else if (req.url.includes("/stream")) {
              respBody = new ReadableStream({
                start(controller) {
                  controller.enqueue(new Uint8Array([1]));
                },
                cancel(reason) {
                  streamCancelled!.resolve(reason);
                },
              });
            } else {
              fail();
            }
            waitForRequest.resolve();
            await waitForAbort.promise;

            if (delay == "delay") {
              await new Promise((r) => setTimeout(r, 1000));
            }
            // Allocate the request body
            req.body;
            return new Response(respBody);
          },
        });

        await listeningDeferred.promise;

        // Create a POST request and drop it once the server has received it
        const conn = await Deno.connect({ port: servePort });
        const writer = conn.writable.getWriter();
        await writer.write(
          new TextEncoder().encode(`POST /${url} HTTP/1.0\n\n`),
        );
        await waitForRequest.promise;
        await writer.close();

        waitForAbort.resolve();

        // Wait for cancellation before we shut the server down
        if (streamCancelled !== undefined) {
          await streamCancelled;
        }

        // Since the handler has a chance of creating resources or running async
        // ops, we need to use a graceful shutdown here to ensure they have fully
        // drained.
        await server.shutdown();

        await server.finished;
      },
    });
  }
}

// Test for the internal implementation detail of cached request signals. Ensure that the request's
// signal is aborted if we try to access it after the request has been completed.
Deno.test(
  { permissions: { net: true } },
  async function httpServerSignalCancelled() {
    let stashedRequest;
    const { finished, abort } = await makeServer((req) => {
      // The cache signal is `undefined` because it has not been requested
      assertEquals(getCachedAbortSignal(req), undefined);
      stashedRequest = req;
      return new Response("ok");
    });
    await (await fetch(`http://localhost:${servePort}`)).text();
    abort();
    await finished;

    // `false` is a semaphore for a signal that should be aborted on creation
    assertEquals(getCachedAbortSignal(stashedRequest!), false);
    // Requesting the signal causes it to be materialized
    assert(stashedRequest!.signal.aborted);
    // The cached signal is now a full `AbortSignal`
    assertEquals(
      getCachedAbortSignal(stashedRequest!).constructor,
      AbortSignal,
    );
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerCancelFetch() {
    const request2 = Promise.withResolvers<void>();
    const request2Aborted = Promise.withResolvers<string>();
    let completed = 0;
    let aborted = 0;
    const { finished, abort } = await makeServer(async (req, context) => {
      context.completed.then(() => {
        console.log("completed");
        completed++;
      }).catch(() => {
        console.log("completed (error)");
        completed++;
      });
      req.signal.onabort = () => {
        console.log("aborted", req.url);
        aborted++;
      };
      if (req.url.endsWith("/1")) {
        const fetchRecursive = await fetch(`http://localhost:${servePort}/2`);
        return new Response(fetchRecursive.body);
      } else if (req.url.endsWith("/2")) {
        request2.resolve();
        return new Response(
          new ReadableStream({
            start(_controller) {/* just hang */},
            cancel(reason) {
              request2Aborted.resolve(reason);
            },
          }),
        );
      }
      fail();
    });
    const fetchAbort = new AbortController();
    const fetchPromise = await fetch(`http://localhost:${servePort}/1`, {
      signal: fetchAbort.signal,
    });
    await fetchPromise;
    await request2.promise;
    fetchAbort.abort();
    assertEquals("resource closed", await request2Aborted.promise);

    abort();
    await finished;
    assertEquals(completed, 2);
    assertEquals(aborted, 2);
  },
);

// Regression test for https://github.com/denoland/deno/issues/23537
Deno.test(
  { permissions: { read: true, net: true } },
  async function httpServerUndefinedCert() {
    const ac = new AbortController();
    const { promise, resolve } = Promise.withResolvers<void>();
    const hostname = "127.0.0.1";

    const server = Deno.serve({
      handler: () => new Response("Hello World"),
      hostname,
      port: servePort,
      signal: ac.signal,
      onListen: onListen(resolve),
      onError: createOnErrorCb(ac),
      // Undefined should be equivalent to missing
      cert: undefined,
      key: undefined,
    });

    await promise;
    const resp = await fetch(`http://localhost:${servePort}/`);

    const respBody = await resp.text();
    assertEquals("Hello World", respBody);

    ac.abort();
    await server.finished;
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function httpServerWithTls() {
    const ac = new AbortController();
    const { promise, resolve } = Promise.withResolvers<void>();
    const hostname = "127.0.0.1";

    const server = Deno.serve({
      handler: () => new Response("Hello World"),
      hostname,
      port: servePort,
      signal: ac.signal,
      onListen: onListen(resolve),
      onError: createOnErrorCb(ac),
      cert: Deno.readTextFileSync("tests/testdata/tls/localhost.crt"),
      key: Deno.readTextFileSync("tests/testdata/tls/localhost.key"),
    });

    await promise;
    const caCert = Deno.readTextFileSync("tests/testdata/tls/RootCA.pem");
    const client = Deno.createHttpClient({ caCerts: [caCert] });
    const resp = await fetch(`https://localhost:${servePort}/`, {
      client,
      headers: { "connection": "close" },
    });

    const respBody = await resp.text();
    assertEquals("Hello World", respBody);

    client.close();
    ac.abort();
    await server.finished;
  },
);

Deno.test(
  { permissions: { net: true, write: true, read: true } },
  async function httpServerRequestCLTE() {
    const ac = new AbortController();
    const listeningDeferred = Promise.withResolvers<void>();
    const deferred = Promise.withResolvers<void>();

    const server = Deno.serve({
      handler: async (req) => {
        assertEquals(await req.text(), "");
        deferred.resolve();
        return new Response("ok");
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningDeferred.resolve),
      onError: createOnErrorCb(ac),
    });

    await listeningDeferred.promise;
    const conn = await Deno.connect({ port: servePort });
    const encoder = new TextEncoder();

    const body =
      `POST / HTTP/1.1\r\nHost: example.domain\r\nContent-Length: 13\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\nEXTRA`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);
    await deferred.promise;

    conn.close();

    ac.abort();
    await server.finished;
  },
);

Deno.test(
  { permissions: { net: true, write: true, read: true } },
  async function httpServerRequestTETE() {
    const ac = new AbortController();
    const { promise, resolve } = Promise.withResolvers<void>();

    const server = Deno.serve({
      handler: () => {
        throw new Error("oops");
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(resolve),
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

    await promise;
    for (const teHeader of variations) {
      const conn = await Deno.connect({ port: servePort });
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
    await server.finished;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServer204ResponseDoesntSendContentLength() {
    const { promise, resolve } = Promise.withResolvers<void>();
    const ac = new AbortController();
    const server = Deno.serve({
      handler: (_request) => new Response(null, { status: 204 }),
      port: servePort,
      signal: ac.signal,
      onListen: onListen(resolve),
      onError: createOnErrorCb(ac),
    });

    try {
      await promise;
      const resp = await fetch(`http://127.0.0.1:${servePort}/`, {
        method: "GET",
        headers: { "connection": "close" },
      });
      assertEquals(resp.status, 204);
      assertEquals(resp.headers.get("Content-Length"), null);
    } finally {
      ac.abort();
      await server.finished;
    }
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServer304ResponseDoesntSendBody() {
    const deferred = Promise.withResolvers<void>();
    const ac = new AbortController();
    const listeningDeferred = Promise.withResolvers<void>();

    const server = Deno.serve({
      handler: () => {
        deferred.resolve();
        return new Response(null, { status: 304 });
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningDeferred.resolve),
      onError: createOnErrorCb(ac),
    });

    await listeningDeferred.promise;
    const conn = await Deno.connect({ port: servePort });
    const encoder = new TextEncoder();
    const decoder = new TextDecoder();

    const body =
      `GET / HTTP/1.1\r\nHost: example.domain\r\nConnection: close\r\n\r\n`;
    const writeResult = await conn.write(encoder.encode(body));
    assertEquals(body.length, writeResult);

    await deferred.promise;

    const buf = new Uint8Array(1024);
    const readResult = await conn.read(buf);
    assert(readResult);
    const msg = decoder.decode(buf.subarray(0, readResult));

    assert(msg.startsWith("HTTP/1.1 304 Not Modified"));
    assert(msg.endsWith("\r\n\r\n"));

    conn.close();

    ac.abort();
    await server.finished;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerExpectContinue() {
    const deferred = Promise.withResolvers<void>();
    const ac = new AbortController();
    const listeningDeferred = Promise.withResolvers<void>();

    const server = Deno.serve({
      handler: async (req) => {
        deferred.resolve();
        assertEquals(await req.text(), "hello");
        return new Response(null, { status: 304 });
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningDeferred.resolve),
      onError: createOnErrorCb(ac),
    });

    await listeningDeferred.promise;
    const conn = await Deno.connect({ port: servePort });
    const encoder = new TextEncoder();
    const decoder = new TextDecoder();

    {
      const body =
        `POST / HTTP/1.1\r\nHost: example.domain\r\nExpect: 100-continue\r\nContent-Length: 5\r\nConnection: close\r\n\r\n`;
      const writeResult = await conn.write(encoder.encode(body));
      assertEquals(body.length, writeResult);
    }

    await deferred.promise;

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
    await server.finished;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function httpServerExpectContinueButNoBodyLOL() {
    const deferred = Promise.withResolvers<void>();
    const listeningDeferred = Promise.withResolvers<void>();
    const ac = new AbortController();

    const server = Deno.serve({
      handler: async (req) => {
        deferred.resolve();
        assertEquals(await req.text(), "");
        return new Response(null, { status: 304 });
      },
      port: servePort,
      signal: ac.signal,
      onListen: onListen(listeningDeferred.resolve),
      onError: createOnErrorCb(ac),
    });

    await listeningDeferred.promise;
    const conn = await Deno.connect({ port: servePort });
    const encoder = new TextEncoder();
    const decoder = new TextDecoder();

    {
      // // no content-length or transfer-encoding means no body!
      const body =
        `POST / HTTP/1.1\r\nHost: example.domain\r\nExpect: 100-continue\r\nConnection: close\r\n\r\n`;
      const writeResult = await conn.write(encoder.encode(body));
      assertEquals(body.length, writeResult);
    }

    await deferred.promise;

    const buf = new Uint8Array(1024);
    const readResult = await conn.read(buf);
    assert(readResult);
    const msg = decoder.decode(buf.subarray(0, readResult));

    assert(msg.startsWith("HTTP/1.1 304 Not Modified"));
    conn.close();

    ac.abort();
    await server.finished;
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
      const { promise, resolve } = Promise.withResolvers<void>();

      const server = Deno.serve({
        handler: () => {
          throw new Error("oops");
        },
        port: servePort,
        signal: ac.signal,
        onListen: onListen(resolve),
        onError: createOnErrorCb(ac),
      });

      await promise;
      const conn = await Deno.connect({ port: servePort });
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
      await server.finished;
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
    const { resolve } = Promise.withResolvers<void>();

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
      port: servePort,
      signal: ac.signal,
      onListen: onListen(resolve),
      onError: createOnErrorCb(ac),
    });

    const sseRequest = await fetch(`http://localhost:${servePort}/`);

    const decoder = new TextDecoder();
    const stream = sseRequest.body!.getReader();
    {
      const { done, value } = await stream.read();
      assert(!done);
      assertEquals(decoder.decode(value), "data: hello\r\n\r\n");
    }

    const helloRequest = await fetch(`http://localhost:${servePort}/`);
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
    await server.finished;
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
      const { resolve } = Promise.withResolvers<void>();
      const server = Deno.serve({
        handler: (_req) => new Response("ok"),
        hostname: "localhost",
        port: servePort,
        signal: ac.signal,
        onListen: onListen(resolve),
        onError: createOnErrorCb(ac),
      });
      ac.abort();
      await server.finished;
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
    const { promise, resolve } = Promise.withResolvers<void>();
    let count = 0;
    const server = Deno.serve({
      async onListen({ port }: { port: number }) {
        const res1 = await fetch(`http://localhost:${port}/`);
        assertEquals(await res1.text(), "hello world 1");

        const res2 = await fetch(`http://localhost:${port}/`);
        assertEquals(await res2.text(), "hello world 2");

        resolve();
        ac.abort();
      },
      signal: ac.signal,
    }, () => {
      count++;
      return new Response(`hello world ${count}`);
    });

    await promise;
    await server.finished;
  },
);

// https://github.com/denoland/deno/issues/15858
Deno.test(
  "Clone should work",
  { permissions: { net: true } },
  async function httpServerCanCloneRequest() {
    const ac = new AbortController();
    const { promise, resolve } = Promise.withResolvers<number>();

    const server = Deno.serve({
      handler: async (req) => {
        const cloned = req.clone();
        assertEquals(req.headers, cloned.headers);

        assertEquals(cloned.url, req.url);
        assertEquals(cloned.cache, req.cache);
        assertEquals(cloned.destination, req.destination);
        assertEquals(cloned.headers, req.headers);
        assertEquals(cloned.integrity, req.integrity);
        assertEquals(cloned.isHistoryNavigation, req.isHistoryNavigation);
        assertEquals(cloned.isReloadNavigation, req.isReloadNavigation);
        assertEquals(cloned.keepalive, req.keepalive);
        assertEquals(cloned.method, req.method);
        assertEquals(cloned.mode, req.mode);
        assertEquals(cloned.redirect, req.redirect);
        assertEquals(cloned.referrer, req.referrer);
        assertEquals(cloned.referrerPolicy, req.referrerPolicy);

        // both requests can read body
        await req.text();
        await cloned.json();

        return new Response("ok");
      },
      signal: ac.signal,
      onListen: ({ port }: { port: number }) => resolve(port),
      onError: createOnErrorCb(ac),
    });

    try {
      const port = await promise;
      const resp = await fetch(`http://localhost:${port}/`, {
        headers: { connection: "close" },
        method: "POST",
        body: '{"sus":true}',
      });
      const text = await resp.text();
      assertEquals(text, "ok");
    } finally {
      ac.abort();
      await server.finished;
    }
  },
);

// https://fetch.spec.whatwg.org/#dom-request-clone
Deno.test(
  "Throw if disturbed",
  { permissions: { net: true } },
  async function shouldThrowIfBodyIsUnusableDisturbed() {
    const ac = new AbortController();
    const { promise, resolve } = Promise.withResolvers<number>();

    const server = Deno.serve({
      handler: async (req) => {
        await req.text();

        try {
          req.clone();
          fail();
        } catch (cloneError) {
          assert(cloneError instanceof TypeError);
          assert(
            cloneError.message.endsWith("Body is unusable."),
          );

          ac.abort();
          await server.finished;
        }

        return new Response("ok");
      },
      signal: ac.signal,
      onListen: ({ port }: { port: number }) => resolve(port),
    });

    try {
      const port = await promise;
      await fetch(`http://localhost:${port}/`, {
        headers: { connection: "close" },
        method: "POST",
        body: '{"bar":true}',
      });
      fail();
    } catch (clientError) {
      assert(clientError instanceof TypeError);
      assert(clientError.message.includes("client error"));
    } finally {
      ac.abort();
      await server.finished;
    }
  },
);

// https://fetch.spec.whatwg.org/#dom-request-clone
Deno.test({
  name: "Throw if locked",
  permissions: { net: true },
  fn: async function shouldThrowIfBodyIsUnusableLocked() {
    const ac = new AbortController();
    const { promise, resolve } = Promise.withResolvers<number>();

    const server = Deno.serve({
      handler: async (req) => {
        const _reader = req.body?.getReader();

        try {
          req.clone();
          fail();
        } catch (cloneError) {
          assert(cloneError instanceof TypeError);
          assert(
            cloneError.message.endsWith("Body is unusable."),
          );

          ac.abort();
          await server.finished;
        }
        return new Response("ok");
      },
      signal: ac.signal,
      onListen: ({ port }: { port: number }) => resolve(port),
    });

    try {
      const port = await promise;
      await fetch(`http://localhost:${port}/`, {
        headers: { connection: "close" },
        method: "POST",
        body: '{"bar":true}',
      });
      fail();
    } catch (clientError) {
      assert(clientError instanceof TypeError);
      assert(clientError.message.includes("client error"));
    } finally {
      ac.abort();
      await server.finished;
    }
  },
});

// Checks large streaming response
// https://github.com/denoland/deno/issues/16567
Deno.test(
  { permissions: { net: true } },
  async function testIssue16567() {
    const ac = new AbortController();
    const { promise, resolve } = Promise.withResolvers<void>();
    const server = Deno.serve({
      async onListen({ port }) {
        const res1 = await fetch(`http://localhost:${port}/`);
        assertEquals((await res1.text()).length, 40 * 50_000);

        resolve();
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
    await server.finished;
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

// TODO(mmastrac): curl on Windows CI stopped supporting --http2?
Deno.test(
  {
    permissions: { net: true, run: true },
    ignore: Deno.build.os === "windows",
  },
  async function httpServeCurlH2C() {
    const ac = new AbortController();
    const server = Deno.serve(
      { port: servePort, signal: ac.signal },
      () => new Response("hello world!"),
    );

    assertEquals(
      "hello world!",
      await curlRequest([`http://localhost:${servePort}/path`]),
    );
    assertEquals(
      "hello world!",
      await curlRequest([`http://localhost:${servePort}/path`, "--http2"]),
    );
    assertEquals(
      "hello world!",
      await curlRequest([
        `http://localhost:${servePort}/path`,
        "--http2",
        "--http2-prior-knowledge",
      ]),
    );

    ac.abort();
    await server.finished;
  },
);

// TODO(mmastrac): This test should eventually use fetch, when we support trailers there.
// This test is ignored because it's flaky and relies on cURL's verbose output.
Deno.test(
  { permissions: { net: true, run: true, read: true }, ignore: true },
  async function httpServerTrailers() {
    const ac = new AbortController();
    const { resolve } = Promise.withResolvers<void>();

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
      onListen: onListen(resolve),
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
    await server.finished;
  },
);

// TODO(mmastrac): curl on CI stopped supporting --http2?
Deno.test(
  {
    permissions: {
      net: true,
      run: true,
      read: true,
    },
    ignore: Deno.build.os === "windows",
  },
  async function httpsServeCurlH2C() {
    const ac = new AbortController();
    const server = Deno.serve(
      {
        signal: ac.signal,
        port: servePort,
        cert: Deno.readTextFileSync("tests/testdata/tls/localhost.crt"),
        key: Deno.readTextFileSync("tests/testdata/tls/localhost.key"),
      },
      () => new Response("hello world!"),
    );

    assertEquals(
      "hello world!",
      await curlRequest([`https://localhost:${servePort}/path`, "-k"]),
    );
    assertEquals(
      "hello world!",
      await curlRequest([
        `https://localhost:${servePort}/path`,
        "-k",
        "--http2",
      ]),
    );
    assertEquals(
      "hello world!",
      await curlRequest([
        `https://localhost:${servePort}/path`,
        "-k",
        "--http2",
        "--http2-prior-knowledge",
      ]),
    );

    ac.abort();
    await server.finished;
  },
);

Deno.test("Deno.HttpServer is not thenable", async () => {
  // deno-lint-ignore require-await
  async function serveTest() {
    const server = Deno.serve({ port: servePort }, (_) => new Response(""));
    assert(!("then" in server));
    return server;
  }
  const server = await serveTest();
  await server.shutdown();
});

Deno.test(
  {
    ignore: Deno.build.os === "windows",
    permissions: { run: true, read: true, write: true },
  },
  async function httpServerUnixDomainSocket() {
    const { promise, resolve } = Promise.withResolvers<Deno.UnixAddr>();
    const ac = new AbortController();
    const filePath = tmpUnixSocketPath();
    const server = Deno.serve(
      {
        signal: ac.signal,
        path: filePath,
        onListen(info) {
          resolve(info);
        },
        onError: createOnErrorCb(ac),
      },
      (_req, { remoteAddr }) => {
        assertEquals(remoteAddr, { path: filePath, transport: "unix" });
        return new Response("hello world!");
      },
    );

    assertEquals((await promise).path, filePath);
    assertEquals(
      "hello world!",
      await curlRequest(["--unix-socket", filePath, "http://localhost"]),
    );
    ac.abort();
    await server.finished;
  },
);

// serve Handler must return Response class or promise that resolves Response class
Deno.test(
  { permissions: { net: true, run: true } },
  async function handleServeCallbackReturn() {
    const deferred = Promise.withResolvers<void>();
    const listeningDeferred = Promise.withResolvers<void>();
    const ac = new AbortController();

    const server = Deno.serve(
      {
        port: servePort,
        onListen: onListen(listeningDeferred.resolve),
        signal: ac.signal,
        onError: (error) => {
          assert(error instanceof TypeError);
          assert(
            error.message ===
              "Return value from serve handler must be a response or a promise resolving to a response",
          );
          deferred.resolve();
          return new Response("Customized Internal Error from onError");
        },
      },
      () => {
        // Trick the typechecker
        return <Response> <unknown> undefined;
      },
    );
    await listeningDeferred.promise;
    const respText = await curlRequest([`http://localhost:${servePort}`]);
    await deferred.promise;
    ac.abort();
    await server.finished;
    assert(respText === "Customized Internal Error from onError");
  },
);

// onError Handler must return Response class or promise that resolves Response class
Deno.test(
  { permissions: { net: true, run: true } },
  async function handleServeErrorCallbackReturn() {
    const { promise, resolve } = Promise.withResolvers<void>();
    const ac = new AbortController();

    const server = Deno.serve(
      {
        port: servePort,
        onListen: onListen(resolve),
        signal: ac.signal,
        onError: () => {
          // Trick the typechecker
          return <Response> <unknown> undefined;
        },
      },
      () => {
        // Trick the typechecker
        return <Response> <unknown> undefined;
      },
    );
    await promise;
    const respText = await curlRequest([`http://localhost:${servePort}`]);
    ac.abort();
    await server.finished;
    assert(respText === "Internal Server Error");
  },
);

Deno.test(
  {
    permissions: { net: true, run: true, read: true },
    ignore: Deno.build.os !== "linux",
  },
  async function gzipFlushResponseStream() {
    const { promise, resolve } = Promise.withResolvers<void>();
    const ac = new AbortController();

    console.log("Starting server", servePort);
    let timer: number | undefined = undefined;
    let _controller;

    const server = Deno.serve(
      {
        port: servePort,
        onListen: onListen(resolve),
        signal: ac.signal,
      },
      () => {
        const body = new ReadableStream({
          start(controller) {
            timer = setInterval(() => {
              const message = `It is ${new Date().toISOString()}\n`;
              controller.enqueue(new TextEncoder().encode(message));
            }, 1000);
            _controller = controller;
          },
          cancel() {
            if (timer !== undefined) {
              clearInterval(timer);
            }
          },
        });
        return new Response(body, {
          headers: {
            "content-type": "text/plain",
            "x-content-type-options": "nosniff",
          },
        });
      },
    );
    await promise;
    const e = await execCode3("/usr/bin/sh", [
      "-c",
      `curl --stderr - -N --compressed --no-progress-meter http://localhost:${servePort}`,
    ]);
    await e.waitStdoutText("It is ");
    clearTimeout(timer);
    _controller!.close();
    await e.finished();
    ac.abort();
    await server.finished;
  },
);

Deno.test({
  name: "HTTP Server test (error on non-unix platform)",
  ignore: Deno.build.os !== "windows",
}, async () => {
  await assertRejects(
    async () => {
      const ac = new AbortController();
      const server = Deno.serve({
        path: "path/to/socket",
        handler: (_req) => new Response("Hello, world"),
        signal: ac.signal,
        onListen({ path: _path }) {
          console.log(`Server started at ${_path}`);
        },
      });
      server.finished.then(() => console.log("Server closed"));
      console.log("Closing server...");
      ac.abort();
      await new Promise((resolve) => setTimeout(resolve, 100)); // Example of awaiting something
    },
    Error,
    'Operation `"op_net_listen_unix"` not supported on non-unix platforms.',
  );
});
