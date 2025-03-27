// Copyright 2018-2025 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

import EventEmitter from "node:events";
import http, {
  IncomingMessage,
  type RequestOptions,
  ServerResponse,
} from "node:http";
import url from "node:url";
import https from "node:https";
import zlib from "node:zlib";
import net, { Socket } from "node:net";
import fs from "node:fs";
import { text } from "node:stream/consumers";

import { assert, assertEquals, assertStringIncludes, fail } from "@std/assert";
import { assertSpyCalls, spy } from "@std/testing/mock";
import { fromFileUrl, relative } from "@std/path";
import { retry } from "@std/async/retry";

import { gzip } from "node:zlib";
import { Buffer } from "node:buffer";
import { execCode } from "../unit/test_util.ts";

Deno.test("[node/http listen]", async () => {
  {
    const server = http.createServer();
    assertEquals(0, EventEmitter.listenerCount(server, "request"));
  }

  {
    const server = http.createServer(() => {});
    assertEquals(1, EventEmitter.listenerCount(server, "request"));
  }

  {
    const { promise, resolve } = Promise.withResolvers<void>();
    const server = http.createServer();

    server.listen(42453, "localhost", () => {
      // @ts-ignore address() is not a string
      assertEquals(server.address()!.address, "127.0.0.1");
      server.close();
    });
    server.on("close", () => {
      resolve();
    });

    await promise;
  }

  {
    const { promise, resolve } = Promise.withResolvers<void>();
    const server = http.createServer();

    server.listen().on("listening", () => {
      server.close();
    });
    server.on("close", () => {
      resolve();
    });

    await promise;
  }

  for (const port of [0, -0, 0.0, "0", null, undefined]) {
    const { promise, resolve } = Promise.withResolvers<void>();
    const server = http.createServer();

    server.listen(port, () => {
      server.close();
    });
    server.on("close", () => {
      resolve();
    });

    await promise;
  }
});

Deno.test("[node/http close]", async () => {
  {
    const deferred1 = Promise.withResolvers<void>();
    const deferred2 = Promise.withResolvers<void>();
    // Node quirk: callback gets exception object, event listener does not.
    // deno-lint-ignore no-explicit-any
    const server = http.createServer().close((err: any) => {
      assertEquals(err.code, "ERR_SERVER_NOT_RUNNING");
      deferred1.resolve();
    });
    // deno-lint-ignore no-explicit-any
    server.on("close", (err: any) => {
      assertEquals(err, undefined);
      deferred2.resolve();
    });
    server.on("listening", () => {
      throw Error("unreachable");
    });
    await deferred1.promise;
    await deferred2.promise;
  }

  {
    const deferred1 = Promise.withResolvers<void>();
    const deferred2 = Promise.withResolvers<void>();
    const server = http.createServer().listen().close((err) => {
      assertEquals(err, undefined);
      deferred1.resolve();
    });
    // deno-lint-ignore no-explicit-any
    server.on("close", (err: any) => {
      assertEquals(err, undefined);
      deferred2.resolve();
    });
    server.on("listening", () => {
      throw Error("unreachable");
    });
    await deferred1.promise;
    await deferred2.promise;
  }
});

Deno.test("[node/http] chunked response", async () => {
  for (
    const body of [undefined, "", "ok"]
  ) {
    const expected = body ?? "";
    const { promise, resolve } = Promise.withResolvers<void>();

    const server = http.createServer((_req, res) => {
      res.writeHead(200, { "transfer-encoding": "chunked" });
      res.end(body);
    });

    server.listen(async () => {
      const res = await fetch(
        // deno-lint-ignore no-explicit-any
        `http://127.0.0.1:${(server.address() as any).port}/`,
      );
      assert(res.ok);

      const actual = await res.text();
      assertEquals(actual, expected);

      server.close(() => resolve());
    });

    await promise;
  }
});

Deno.test("[node/http] .writeHead()", async (t) => {
  async function testWriteHead(
    onRequest: (res: ServerResponse) => void,
    onResponse: (res: Response) => void,
  ) {
    const { promise, resolve } = Promise.withResolvers<void>();
    const server = http.createServer((_req, res) => {
      onRequest(res);
      res.end();
    });
    server.listen(async () => {
      const res = await fetch(
        // deno-lint-ignore no-explicit-any
        `http://127.0.0.1:${(server.address() as any).port}/`,
      );
      await res.body?.cancel();

      onResponse(res);

      server.close(() => resolve());
    });

    await promise;
  }

  await t.step("send status code", async () => {
    await testWriteHead(
      (res) => res.writeHead(404),
      (res) => {
        assertEquals(res.status, 404);
      },
    );
  });

  // TODO(@marvinhagemeister): hyper doesn't support custom status text
  // await t.step("send status + custom status text", async () => {
  //   await testWriteHead(
  //     (res) => res.writeHead(404, "some text"),
  //     (res) => {
  //       assertEquals(res.status, 404);
  //       assertEquals(res.statusText, "some text");
  //     },
  //   );
  // });

  await t.step("send status + custom status text + headers obj", async () => {
    await testWriteHead(
      (res) => res.writeHead(404, "some text", { foo: "bar" }),
      (res) => {
        assertEquals(res.status, 404);
        // TODO(@marvinhagemeister): hyper doesn't support custom
        // status text
        // assertEquals(res.statusText, "some text");
        assertEquals(res.headers.get("foo"), "bar");
      },
    );
  });

  await t.step("send status + headers obj", async () => {
    await testWriteHead(
      (res) => {
        res.writeHead(200, {
          foo: "bar",
          bar: ["foo1", "foo2"],
          foobar: 1,
        });
      },
      (res) => {
        assertEquals(res.status, 200);
        assertEquals(res.headers.get("foo"), "bar");
        assertEquals(res.headers.get("bar"), "foo1, foo2");
        assertEquals(res.headers.get("foobar"), "1");
      },
    );
  });

  await t.step("send status + headers array", async () => {
    await testWriteHead(
      (res) => res.writeHead(200, [["foo", "bar"]]),
      (res) => {
        assertEquals(res.status, 200);
        assertEquals(res.headers.get("foo"), "bar");
      },
    );
  });
});

// Test empty chunks: https://github.com/denoland/deno/issues/17194
Deno.test("[node/http] empty chunk in the middle of response", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();

  const server = http.createServer((_req, res) => {
    res.write("a");
    res.write("");
    res.write("b");
    res.end();
  });

  server.listen(async () => {
    const res = await fetch(
      // deno-lint-ignore no-explicit-any
      `http://127.0.0.1:${(server.address() as any).port}/`,
    );
    const actual = await res.text();
    assertEquals(actual, "ab");
    server.close(() => resolve());
  });

  await promise;
});

Deno.test("[node/http] server can respond with 101, 204, 205, 304 status", async () => {
  for (const status of [101, 204, 205, 304]) {
    const { promise, resolve } = Promise.withResolvers<void>();
    const server = http.createServer((_req, res) => {
      res.statusCode = status;
      res.end("");
    });
    server.listen(async () => {
      const res = await fetch(
        // deno-lint-ignore no-explicit-any
        `http://127.0.0.1:${(server.address() as any).port}/`,
      );
      await res.body?.cancel();
      assertEquals(res.status, status);
      server.close(() => resolve());
    });
    await promise;
  }
});

Deno.test("[node/http] multiple set-cookie headers", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();

  const server = http.createServer((_req, res) => {
    res.setHeader("Set-Cookie", ["foo=bar", "bar=foo"]);
    assertEquals(res.getHeader("Set-Cookie"), ["foo=bar", "bar=foo"]);
    res.end();
  });

  server.listen(async () => {
    const res = await fetch(
      // deno-lint-ignore no-explicit-any
      `http://127.0.0.1:${(server.address() as any).port}/`,
    );
    assert(res.ok);

    const setCookieHeaders = res.headers.getSetCookie();
    assertEquals(setCookieHeaders, ["foo=bar", "bar=foo"]);

    await res.body!.cancel();

    server.close(() => resolve());
  });

  await promise;
});

Deno.test("[node/http] IncomingRequest socket has remoteAddress + remotePort", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();

  let remoteAddress: string | undefined;
  let remotePort: number | undefined;
  const server = http.createServer((req, res) => {
    remoteAddress = req.socket.remoteAddress;
    remotePort = req.socket.remotePort;
    res.end();
  });
  server.listen(async () => {
    // deno-lint-ignore no-explicit-any
    const port = (server.address() as any).port;
    const res = await fetch(
      `http://127.0.0.1:${port}/`,
    );
    await res.arrayBuffer();
    assertEquals(remoteAddress, "127.0.0.1");
    assertEquals(typeof remotePort, "number");
    server.close(() => resolve());
  });
  await promise;
});

Deno.test("[node/http] request default protocol", async () => {
  const deferred1 = Promise.withResolvers<void>();
  const deferred2 = Promise.withResolvers<void>();
  const server = http.createServer((_, res) => {
    res.end("ok");
  });

  // @ts-ignore IncomingMessageForClient
  // deno-lint-ignore no-explicit-any
  let clientRes: any;
  // deno-lint-ignore no-explicit-any
  let clientReq: any;
  server.listen(() => {
    clientReq = http.request(
      // deno-lint-ignore no-explicit-any
      { host: "localhost", port: (server.address() as any).port },
      (res) => {
        assert(res.socket instanceof EventEmitter);
        assertEquals(res.complete, false);
        res.on("data", () => {});
        res.on("end", () => {
          server.close();
        });
        clientRes = res;
        assertEquals(res.statusCode, 200);
        deferred2.resolve();
      },
    );
    clientReq.end();
  });
  server.on("close", () => {
    deferred1.resolve();
  });
  await deferred1.promise;
  await deferred2.promise;
  assert(clientReq.socket instanceof EventEmitter);
  assertEquals(clientRes!.complete, true);
});

Deno.test("[node/http] request non-ws upgrade header", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const server = http.createServer((_req, res) => {
    res.writeHead(200, { "upgrade": "h2,h2c" });
    res.end("ok");
  });
  server.listen(() => {
    const req = http.request(
      {
        host: "localhost",
        // deno-lint-ignore no-explicit-any
        port: (server.address() as any).port,
      },
      (res) => {
        res.on("data", () => {});
        res.on("end", () => {
          server.close();
        });
        assertEquals(res.statusCode, 200);
      },
    );
    req.end();
  });
  server.on("close", () => {
    resolve();
  });
  await promise;
});

Deno.test("[node/http] request with headers", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const server = http.createServer((req, res) => {
    assertEquals(req.headers["x-foo"], "bar");
    res.end("ok");
  });
  server.listen(() => {
    const req = http.request(
      {
        host: "localhost",
        // deno-lint-ignore no-explicit-any
        port: (server.address() as any).port,
        headers: { "x-foo": "bar" },
      },
      (res) => {
        res.on("data", () => {});
        res.on("end", () => {
          server.close();
        });
        assertEquals(res.statusCode, 200);
      },
    );
    req.end();
  });
  server.on("close", () => {
    resolve();
  });
  await promise;
});

Deno.test("[node/http] non-string buffer response", {
  // TODO(kt3k): Enable sanitizer. A "zlib" resource is leaked in this test case.
  sanitizeResources: false,
}, async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const server = http.createServer((_, res) => {
    res.socket!.end();
    gzip(
      Buffer.from("a".repeat(100), "utf8"),
      {},
      (_err: Error | null, data: Buffer) => {
        res.setHeader("Content-Encoding", "gzip");
        res.end(data);
      },
    );
  });
  server.listen(async () => {
    const res = await fetch(
      // deno-lint-ignore no-explicit-any
      `http://localhost:${(server.address() as any).port}`,
    );
    try {
      const text = await res.text();
      assertEquals(text, "a".repeat(100));
    } catch (e) {
      server.emit("error", e);
    } finally {
      server.close(() => resolve());
    }
  });
  await promise;
});

// TODO(kt3k): Enable this test
// Currently IncomingMessage constructor has incompatible signature.
/*
Deno.test("[node/http] http.IncomingMessage can be created without url", () => {
  const message = new http.IncomingMessage(
    // adapted from https://github.com/dougmoscrop/serverless-http/blob/80bfb3e940057d694874a8b0bc12ad96d2abe7ab/lib/request.js#L7
    {
      // @ts-expect-error - non-request properties will also be passed in, e.g. by serverless-http
      encrypted: true,
      readable: false,
      remoteAddress: "foo",
      address: () => ({ port: 443 }),
      // deno-lint-ignore no-explicit-any
      end: Function.prototype as any,
      // deno-lint-ignore no-explicit-any
      destroy: Function.prototype as any,
    },
  );
  message.url = "https://example.com";
});
*/

Deno.test("[node/http] send request with non-chunked body", async () => {
  let requestHeaders: Headers;
  let requestBody = "";

  const hostname = "localhost";
  const port = 4505;

  const handler = async (req: Request) => {
    requestHeaders = req.headers;
    requestBody = await req.text();
    return new Response("ok");
  };
  const abortController = new AbortController();
  const servePromise = Deno.serve({
    hostname,
    port,
    signal: abortController.signal,
    onListen: undefined,
  }, handler).finished;

  const opts: RequestOptions = {
    host: hostname,
    port,
    method: "POST",
    headers: {
      "Content-Type": "text/plain; charset=utf-8",
      "Content-Length": "11",
    },
  };
  const req = http.request(opts, (res) => {
    res.on("data", () => {});
    res.on("end", () => {
      abortController.abort();
    });
    assertEquals(res.statusCode, 200);
    assertEquals(requestHeaders.get("content-length"), "11");
    assertEquals(requestHeaders.has("transfer-encoding"), false);
    assertEquals(requestBody, "hello world");
  });
  req.on("socket", (socket) => {
    assert(socket.writable);
    assert(socket.readable);
    socket.setKeepAlive();
    socket.setTimeout(100);
  });
  req.write("hello ");
  req.write("world");
  req.end();

  await Promise.all([
    servePromise,
    // wait 100ms because of the socket.setTimeout(100) above
    // in order to not cause a flaky test sanitizer failure
    await new Promise((resolve) => setTimeout(resolve, 100)),
  ]);

  if (Deno.build.os === "windows") {
    // FIXME(kt3k): This is necessary for preventing op leak on windows
    await new Promise((resolve) => setTimeout(resolve, 4000));
  }
});

Deno.test("[node/http] send request with chunked body", async () => {
  let requestHeaders: Headers;
  let requestBody = "";

  const hostname = "localhost";
  const port = 4505;

  const handler = async (req: Request) => {
    requestHeaders = req.headers;
    requestBody = await req.text();
    return new Response("ok");
  };
  const abortController = new AbortController();
  const servePromise = Deno.serve({
    hostname,
    port,
    signal: abortController.signal,
    onListen: undefined,
  }, handler).finished;

  const opts: RequestOptions = {
    host: hostname,
    port,
    method: "POST",
    headers: {
      "Content-Type": "text/plain; charset=utf-8",
      "Content-Length": "11",
      "Transfer-Encoding": "chunked",
    },
  };
  const req = http.request(opts, (res) => {
    res.on("data", () => {});
    res.on("end", () => {
      abortController.abort();
    });
    assertEquals(res.statusCode, 200);
    assertEquals(requestHeaders.has("content-length"), false);
    assertEquals(requestHeaders.get("transfer-encoding"), "chunked");
    assertEquals(requestBody, "hello world");
  });
  req.write("hello ");
  req.write("world");
  req.end();

  await servePromise;

  if (Deno.build.os === "windows") {
    // FIXME(kt3k): This is necessary for preventing op leak on windows
    await new Promise((resolve) => setTimeout(resolve, 4000));
  }
});

Deno.test("[node/http] send request with chunked body as default", async () => {
  let requestHeaders: Headers;
  let requestBody = "";

  const hostname = "localhost";
  const port = 4505;

  const handler = async (req: Request) => {
    requestHeaders = req.headers;
    requestBody = await req.text();
    return new Response("ok");
  };
  const abortController = new AbortController();
  const servePromise = Deno.serve({
    hostname,
    port,
    signal: abortController.signal,
    onListen: undefined,
  }, handler).finished;

  const opts: RequestOptions = {
    host: hostname,
    port,
    method: "POST",
    headers: {
      "Content-Type": "text/plain; charset=utf-8",
    },
  };
  const req = http.request(opts, (res) => {
    res.on("data", () => {});
    res.on("end", () => {
      abortController.abort();
    });
    assertEquals(res.statusCode, 200);
    assertEquals(requestHeaders.has("content-length"), false);
    assertEquals(requestHeaders.get("transfer-encoding"), "chunked");
    assertEquals(requestBody, "hello world");
  });
  req.write("hello ");
  req.write("world");
  req.end();

  await servePromise;

  if (Deno.build.os === "windows") {
    // FIXME(kt3k): This is necessary for preventing op leak on windows
    await new Promise((resolve) => setTimeout(resolve, 4000));
  }
});

Deno.test("[node/http] ServerResponse _implicitHeader", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const server = http.createServer((_req, res) => {
    const writeHeadSpy = spy(res, "writeHead");
    // deno-lint-ignore no-explicit-any
    (res as any)._implicitHeader();
    assertSpyCalls(writeHeadSpy, 1);
    writeHeadSpy.restore();
    res.end("Hello World");
  });

  server.listen(async () => {
    const { port } = server.address() as { port: number };
    const res = await fetch(`http://localhost:${port}`);
    assertEquals(await res.text(), "Hello World");
    server.close(() => {
      resolve();
    });
  });

  await promise;
});

// https://github.com/denoland/deno/issues/21509
Deno.test("[node/http] ServerResponse flushHeaders", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const server = http.createServer((_req, res) => {
    res.flushHeaders(); // no-op
    res.end("Hello World");
  });

  server.listen(async () => {
    const { port } = server.address() as { port: number };
    const res = await fetch(`http://localhost:${port}`);
    assertEquals(await res.text(), "Hello World");
    server.close(() => {
      resolve();
    });
  });

  await promise;
});

Deno.test("[node/http] server unref", async () => {
  const [statusCode, _output] = await execCode(`
  import http from "node:http";
  const server = http.createServer((_req, res) => {
    res.statusCode = status;
    res.end("");
  });

  // This should let the program to exit without waiting for the
  // server to close.
  server.unref();

  server.listen(async () => {
  });
  `);
  assertEquals(statusCode, 0);
});

Deno.test("[node/http] ClientRequest handle non-string headers", async () => {
  // deno-lint-ignore no-explicit-any
  let headers: any;
  const { promise, resolve, reject } = Promise.withResolvers<void>();
  const req = http.request("http://localhost:4545/echo_server", {
    method: "POST",
    headers: { 1: 2 },
  }, (resp) => {
    headers = resp.headers;

    resp.on("data", () => {});

    resp.on("end", () => {
      resolve();
    });
  });
  req.once("error", (e) => reject(e));
  req.end();
  await promise;
  assertEquals(headers!["1"], "2");
});

Deno.test("[node/https] ClientRequest uses HTTP/1.1", async () => {
  let body = "";
  const { promise, resolve, reject } = Promise.withResolvers<void>();
  const req = https.request("https://localhost:5545/http_version", {
    method: "POST",
    headers: { 1: 2 },
  }, (resp) => {
    resp.on("data", (chunk) => {
      body += chunk;
    });

    resp.on("end", () => {
      resolve();
    });
  });
  req.once("error", (e) => reject(e));
  req.end();
  await promise;
  assertEquals(body, "HTTP/1.1");
});

Deno.test("[node/http] ClientRequest setTimeout", async () => {
  let body = "";
  const { promise, resolve, reject } = Promise.withResolvers<void>();
  const timer = setTimeout(() => reject("timed out"), 50000);
  const req = http.request("http://localhost:4545/http_version", (resp) => {
    resp.on("data", (chunk) => {
      body += chunk;
    });

    resp.on("end", () => {
      resolve();
    });
  });
  req.setTimeout(120000);
  req.once("error", (e) => reject(e));
  req.end();
  await promise;
  clearTimeout(timer);
  assertEquals(body, "HTTP/1.1");
});

Deno.test("[node/http] ClientRequest setNoDelay", async () => {
  let body = "";
  const { promise, resolve, reject } = Promise.withResolvers<void>();
  const timer = setTimeout(() => reject("timed out"), 50000);
  const req = http.request("http://localhost:4545/http_version", (resp) => {
    resp.on("data", (chunk) => {
      body += chunk;
    });

    resp.on("end", () => {
      resolve();
    });
  });
  req.setNoDelay(true);
  req.once("error", (e) => reject(e));
  req.end();
  await promise;
  clearTimeout(timer);
  assertEquals(body, "HTTP/1.1");
});

Deno.test("[node/http] ClientRequest PATCH", async () => {
  let body = "";
  const { promise, resolve, reject } = Promise.withResolvers<void>();
  const req = http.request("http://localhost:4545/echo_server", {
    method: "PATCH",
  }, (resp) => {
    resp.on("data", (chunk) => {
      body += chunk;
    });

    resp.on("end", () => {
      resolve();
    });
  });
  req.write("hello ");
  req.write("world");
  req.once("error", (e) => reject(e));
  req.end();
  await promise;
  assertEquals(body, "hello world");
});

Deno.test("[node/http] ClientRequest PUT", async () => {
  let body = "";
  const { promise, resolve, reject } = Promise.withResolvers<void>();
  const req = http.request("http://localhost:4545/echo_server", {
    method: "PUT",
  }, (resp) => {
    resp.on("data", (chunk) => {
      body += chunk;
    });

    resp.on("end", () => {
      resolve();
    });
  });
  req.write("hello ");
  req.write("world");
  req.once("error", (e) => reject(e));
  req.end();
  await promise;
  assertEquals(body, "hello world");
});

Deno.test("[node/http] ClientRequest search params", async () => {
  let body = "";
  const { promise, resolve, reject } = Promise.withResolvers<void>();
  const req = http.request({
    host: "localhost",
    port: 4545,
    path: "/search_params?foo=bar",
  }, (resp) => {
    resp.on("data", (chunk) => {
      body += chunk;
    });

    resp.on("end", () => {
      resolve();
    });
  });
  req.once("error", (e) => reject(e));
  req.end();
  await promise;
  assertEquals(body, "foo=bar");
});

Deno.test("[node/http] HTTPS server", async () => {
  const deferred = Promise.withResolvers<void>();
  const deferred2 = Promise.withResolvers<void>();
  const client = Deno.createHttpClient({
    caCerts: [Deno.readTextFileSync("tests/testdata/tls/RootCA.pem")],
  });
  const server = https.createServer({
    cert: Deno.readTextFileSync("tests/testdata/tls/localhost.crt"),
    key: Deno.readTextFileSync("tests/testdata/tls/localhost.key"),
  }, (req, res) => {
    // @ts-ignore: It exists on TLSSocket
    assert(req.socket.encrypted);
    res.end("success!");
  });
  server.listen(() => {
    // deno-lint-ignore no-explicit-any
    fetch(`https://localhost:${(server.address() as any).port}`, {
      client,
    }).then(async (res) => {
      assertEquals(res.status, 200);
      assertEquals(await res.text(), "success!");
      server.close();
      deferred2.resolve();
    });
  })
    .on("error", () => fail());
  server.on("close", () => {
    deferred.resolve();
  });
  await Promise.all([deferred.promise, deferred2.promise]);
  client.close();
});

Deno.test(
  "[node/http] client upgrade",
  { permissions: { net: true } },
  async () => {
    const { promise: serverClosed, resolve: resolveServer } = Promise
      .withResolvers<void>();
    const { promise: socketClosed, resolve: resolveSocket } = Promise
      .withResolvers<void>();
    const server = http.createServer((req, res) => {
      // @ts-ignore: It exists on TLSSocket
      assert(!req.socket.encrypted);
      res.writeHead(200, { "Content-Type": "text/plain" });
      res.end("okay");
    });
    // @ts-ignore it's a socket for real
    let serverSocket;
    server.on("upgrade", (req, socket, _head) => {
      // https://github.com/denoland/deno/issues/21979
      assert(req.socket?.write);
      socket.write(
        "HTTP/1.1 101 Web Socket Protocol Handshake\r\n" +
          "Upgrade: WebSocket\r\n" +
          "Connection: Upgrade\r\n" +
          "\r\n",
      );
      serverSocket = socket;
    });

    // Now that server is running
    server.listen(1337, "127.0.0.1", () => {
      // make a request
      const options = {
        port: 1337,
        host: "127.0.0.1",
        headers: {
          "Connection": "Upgrade",
          "Upgrade": "websocket",
        },
      };

      const req = http.request(options);
      req.end();

      req.on("upgrade", (_res, socket, _upgradeHead) => {
        socket.end();
        // @ts-ignore it's a socket for real
        serverSocket!.end();
        server.close(() => {
          resolveServer();
        });
        socket.on("close", () => {
          resolveSocket();
        });
      });
    });

    await serverClosed;
    await socketClosed;
  },
);

Deno.test(
  "[node/http] client end with callback",
  { permissions: { net: true } },
  async () => {
    let received = false;
    const ac = new AbortController();
    const server = Deno.serve({ port: 5928, signal: ac.signal }, (_req) => {
      received = true;
      return new Response("hello");
    });
    const { promise, resolve, reject } = Promise.withResolvers<void>();
    let body = "";

    const request = http.request(
      "http://localhost:5928/",
      (resp) => {
        resp.on("data", (chunk) => {
          body += chunk;
        });

        resp.on("end", () => {
          resolve();
        });
      },
    );
    request.on("error", reject);
    request.end(() => {
      assert(received);
    });

    await promise;
    ac.abort();
    await server.finished;

    assertEquals(body, "hello");
  },
);

Deno.test("[node/http] server emits error if addr in use", async () => {
  const deferred1 = Promise.withResolvers<void>();
  const deferred2 = Promise.withResolvers<Error>();

  const server = http.createServer();
  server.listen(9001);

  const server2 = http.createServer();
  server2.on("error", (e) => {
    deferred2.resolve(e);
  });
  server2.listen(9001);

  const err = await deferred2.promise;
  server.close(() => deferred1.resolve());
  server2.close();
  await deferred1.promise;
  const expectedMsg = Deno.build.os === "windows"
    ? "Only one usage of each socket address"
    : "Address already in use";
  assert(
    err.message.startsWith(expectedMsg),
    `Wrong error: ${err.message}`,
  );
});

Deno.test(
  "[node/http] client destroy doesn't leak",
  { permissions: { net: true } },
  async () => {
    const ac = new AbortController();
    let timerId;

    const server = Deno.serve(
      { port: 5929, signal: ac.signal },
      async (_req) => {
        await new Promise((resolve) => {
          timerId = setTimeout(resolve, 5000);
        });
        return new Response("hello");
      },
    );
    const { promise, resolve, reject } = Promise.withResolvers<void>();

    const request = http.request("http://127.0.0.1:5929/");
    request.on("error", reject);
    request.on("close", () => {});
    request.end();
    setTimeout(() => {
      request.destroy(new Error());
      resolve();
    }, 100);

    await promise;
    clearTimeout(timerId);
    ac.abort();
    await server.finished;
  },
);

Deno.test(
  "[node/http] client destroy before sending request should not error",
  async () => {
    const { resolve, promise } = Promise.withResolvers<void>();
    const request = http.request("http://127.0.0.1:5929/");
    // Calling this would throw
    request.destroy();
    request.on("error", (e) => {
      assertEquals(e.message, "socket hang up");
    });
    request.on("close", () => resolve());
    await promise;

    if (Deno.build.os === "windows") {
      // FIXME(kt3k): This is necessary for preventing op leak on windows
      await new Promise((resolve) => setTimeout(resolve, 4000));
    }
  },
);

const isWindows = Deno.build.os === "windows";

Deno.test(
  "[node/http] destroyed requests should not be sent",
  { sanitizeResources: !isWindows, sanitizeOps: !isWindows },
  async () => {
    let receivedRequest = false;
    const requestClosed = Promise.withResolvers<void>();
    const ac = new AbortController();
    const server = Deno.serve({ port: 0, signal: ac.signal }, () => {
      receivedRequest = true;
      return new Response(null);
    });
    const request = http.request(`http://127.0.0.1:${server.addr.port}/`);
    request.destroy();
    request.end("hello");
    request.on("error", (err) => {
      assert(err.message.includes("socket hang up"));
      ac.abort();
    });
    request.on("close", () => {
      requestClosed.resolve();
    });
    await requestClosed.promise;
    assertEquals(receivedRequest, false);
    await server.finished;
  },
);

Deno.test("[node/http] node:http exports globalAgent", async () => {
  const http = await import("node:http");
  assert(
    http.globalAgent,
    "node:http must export 'globalAgent' on module namespace",
  );
  assert(
    http.default.globalAgent,
    "node:http must export 'globalAgent' on module default export",
  );
});

Deno.test("[node/https] node:https exports globalAgent", async () => {
  const https = await import("node:https");
  assert(
    https.globalAgent,
    "node:https must export 'globalAgent' on module namespace",
  );
  assert(
    https.default.globalAgent,
    "node:https must export 'globalAgent' on module default export",
  );
});

Deno.test("[node/http] node:http request.setHeader(header, null) doesn't throw", async () => {
  {
    const { promise, resolve } = Promise.withResolvers<void>();
    const req = http.request("http://localhost:4545/", (res) => {
      res.on("data", () => {});
      res.on("end", () => {
        resolve();
      });
    });
    // @ts-expect-error - null is not a valid header value
    req.setHeader("foo", null);
    req.end();
    await promise;
  }
  {
    const { promise, resolve } = Promise.withResolvers<void>();
    const req = http.request("http://localhost:4545/", (res) => {
      res.on("data", () => {});
      res.on("end", () => {
        resolve();
      });
    });
    // @ts-expect-error - null is not a valid header value
    req.setHeader("foo", null);
    req.end();

    await promise;
  }
});

Deno.test("[node/http] ServerResponse getHeader", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const server = http.createServer((_req, res) => {
    res.setHeader("foo", "bar");
    assertEquals(res.getHeader("foo"), "bar");
    assertEquals(res.getHeader("ligma"), undefined);
    res.end("Hello World");
  });

  server.listen(async () => {
    const { port } = server.address() as { port: number };
    const res = await fetch(`http://localhost:${port}`);
    assertEquals(await res.text(), "Hello World");
    server.close(() => {
      resolve();
    });
  });

  await promise;
});

Deno.test("[node/http] ServerResponse appendHeader", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const server = http.createServer((_req, res) => {
    res.setHeader("foo", "bar");
    res.appendHeader("foo", "baz");
    res.appendHeader("foo", ["qux"]);
    res.appendHeader("foo", ["quux"]);
    res.appendHeader("Set-Cookie", "a=b");
    res.appendHeader("Set-Cookie", ["c=d", "e=f"]);
    res.end("Hello World");
  });

  server.listen(async () => {
    const { port } = server.address() as { port: number };
    const res = await fetch(`http://localhost:${port}`);
    assertEquals(res.headers.get("foo"), "bar, baz, qux, quux");
    assertEquals(res.headers.getSetCookie(), ["a=b", "c=d", "e=f"]);
    assertEquals(await res.text(), "Hello World");
    server.close(() => {
      resolve();
    });
  });

  await promise;
});

Deno.test("[node/http] ServerResponse appendHeader set-cookie", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const server = http.createServer((_req, res) => {
    res.appendHeader("Set-Cookie", "a=b");
    res.appendHeader("Set-Cookie", "c=d");
    res.end("Hello World");
  });

  server.listen(async () => {
    const { port } = server.address() as { port: number };
    const res = await fetch(`http://localhost:${port}`);
    assertEquals(res.headers.getSetCookie(), ["a=b", "c=d"]);
    assertEquals(await res.text(), "Hello World");
    server.close(() => {
      resolve();
    });
  });

  await promise;
});

Deno.test("[node/http] ServerResponse header names case insensitive", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const server = http.createServer((_req, res) => {
    res.setHeader("Content-Length", "12345");
    assert(res.hasHeader("Content-Length"));
    res.removeHeader("content-length");
    assertEquals(res.getHeader("Content-Length"), undefined);
    assert(!res.hasHeader("Content-Length"));
    res.appendHeader("content-length", "12345");
    res.removeHeader("Content-Length");
    assertEquals(res.getHeader("content-length"), undefined);
    assert(!res.hasHeader("content-length"));
    res.end("Hello World");
  });

  server.listen(async () => {
    const { port } = server.address() as { port: number };
    const res = await fetch(`http://localhost:${port}`);
    assertEquals(res.headers.get("Content-Length"), null);
    assertEquals(res.headers.get("content-length"), null);
    assertEquals(await res.text(), "Hello World");
    server.close(() => {
      resolve();
    });
  });

  await promise;
});

Deno.test("[node/http] IncomingMessage override", () => {
  const req = new http.IncomingMessage(new net.Socket());
  // https://github.com/dougmoscrop/serverless-http/blob/3aaa6d0fe241109a8752efb011c242d249f32368/lib/request.js#L20-L30
  Object.assign(req, {
    ip: "1.1.1.1",
    complete: true,
    httpVersion: "1.1",
    httpVersionMajor: "1",
    httpVersionMinor: "1",
    method: "GET",
    headers: {},
    body: "",
    url: "https://1.1.1.1",
  });
});

Deno.test("[node/http] ServerResponse assignSocket and detachSocket", () => {
  const req = new http.IncomingMessage(new net.Socket());
  const res = new http.ServerResponse(req);
  let writtenData: string | Uint8Array | undefined = undefined;
  let writtenEncoding: string | Uint8Array | undefined = undefined;
  const socket = {
    _writableState: {},
    writable: true,
    on: Function.prototype,
    removeListener: Function.prototype,
    destroy: Function.prototype,
    cork: Function.prototype,
    uncork: Function.prototype,
    write: (
      data: string | Uint8Array,
      encoding: string,
      _cb?: (err?: Error) => void,
    ) => {
      writtenData = data;
      writtenEncoding = encoding;
    },
  };
  // @ts-ignore it's a socket mock
  res.assignSocket(socket);

  res.write("Hello World!", "utf8");
  assertEquals(writtenData, Buffer.from("Hello World!"));
  assertEquals(writtenEncoding, "buffer");

  writtenData = undefined;
  writtenEncoding = undefined;

  // TODO(@littledivy): This test never really worked
  // because there was no data being sent and it passed.
  //
  // @ts-ignore it's a socket mock
  // res.detachSocket(socket);
  // res.write("Hello World!", "utf8");
  //
  // assertEquals(writtenData, undefined);
  // assertEquals(writtenEncoding, undefined);
});

Deno.test("[node/http] ServerResponse getHeaders", () => {
  const req = new http.IncomingMessage(new net.Socket());
  const res = new http.ServerResponse(req);
  res.setHeader("foo", "bar");
  res.setHeader("bar", "baz");
  assertEquals(res.getHeaderNames(), ["foo", "bar"]);
  assertEquals(res.getHeaders(), { "foo": "bar", "bar": "baz" });
});

Deno.test("[node/http] ServerResponse default status code 200", () => {
  const req = new http.IncomingMessage(new net.Socket());
  const res = new http.ServerResponse(req);
  assertEquals(res.statusCode, 200);
});

Deno.test("[node/http] maxHeaderSize is defined", () => {
  assertEquals(http.maxHeaderSize, 16_384);
});

Deno.test("[node/http] server graceful close", async () => {
  const server = http.createServer(function (_, response) {
    response.writeHead(200, {});
    response.end("ok");
    server.close();
  });

  const { promise, resolve } = Promise.withResolvers<void>();
  server.listen(0, function () {
    // deno-lint-ignore no-explicit-any
    const port = (server.address() as any).port;
    const testURL = url.parse(
      `http://localhost:${port}`,
    );

    http.request(testURL, function (response) {
      assertEquals(response.statusCode, 200);
      response.on("data", function () {});
      response.on("end", function () {
        resolve();
      });
    }).end();
  });

  await promise;
});

Deno.test("[node/http] server closeAllConnections shutdown", async () => {
  const server = http.createServer((_req, res) => {
    res.writeHead(200, { "Content-Type": "application/json" });
    res.end(JSON.stringify({
      data: "Hello World!",
    }));
  });

  server.listen(0);
  const { promise, resolve } = Promise.withResolvers<void>();
  setTimeout(() => {
    server.close(() => resolve());
    server.closeAllConnections();
  }, 2000);

  await promise;
});

Deno.test("[node/http] server closeIdleConnections shutdown", async () => {
  const server = http.createServer({ keepAliveTimeout: 60000 }, (_req, res) => {
    res.writeHead(200, { "Content-Type": "application/json" });
    res.end(JSON.stringify({
      data: "Hello World!",
    }));
  });

  server.listen(0);
  const { promise, resolve } = Promise.withResolvers<void>();
  setTimeout(() => {
    server.close(() => resolve());
    server.closeIdleConnections();
  }, 2000);

  await promise;
});

Deno.test("[node/http] client closing a streaming response doesn't terminate server", async () => {
  let interval: number;
  const server = http.createServer((req, res) => {
    res.writeHead(200, { "Content-Type": "text/plain" });
    interval = setInterval(() => {
      res.write("Hello, world!\n");
    }, 100);
    req.on("end", () => {
      clearInterval(interval);
      res.end();
    });
    req.on("error", (err) => {
      console.error("Request error:", err);
      clearInterval(interval);
      res.end();
    });
  });

  const deferred1 = Promise.withResolvers<void>();
  server.listen(0, () => {
    // deno-lint-ignore no-explicit-any
    const port = (server.address() as any).port;

    // Create a client connection to the server
    const client = net.createConnection({ port }, () => {
      console.log("Client connected to server");

      // Write data to the server
      client.write("GET / HTTP/1.1\r\n");
      client.write("Host: localhost\r\n");
      client.write("Connection: close\r\n");
      client.write("\r\n");

      // End the client connection prematurely while reading data
      client.on("data", (data) => {
        assert(data.length > 0);
        client.end();
        setTimeout(() => deferred1.resolve(), 100);
      });
    });
  });

  await deferred1.promise;
  assertEquals(server.listening, true);
  server.close();
  assertEquals(server.listening, false);
  clearInterval(interval!);
});

Deno.test("[node/http] client closing a streaming request doesn't terminate server", async () => {
  let interval: number;
  let uploadedData = "";
  let requestError: Error | null = null;
  const deferred1 = Promise.withResolvers<void>();
  const server = http.createServer((req, res) => {
    res.writeHead(200, { "Content-Type": "text/plain" });
    interval = setInterval(() => {
      res.write("Hello, world!\n");
    }, 100);
    req.on("data", (chunk) => {
      uploadedData += chunk.toString();
    });
    req.on("end", () => {
      clearInterval(interval);
    });
    req.on("error", (err) => {
      deferred1.resolve();
      requestError = err;
      clearInterval(interval);
      res.end();
    });
  });

  server.listen(0, () => {
    // deno-lint-ignore no-explicit-any
    const port = (server.address() as any).port;

    // Create a client connection to the server
    const client = net.createConnection({ port }, () => {
      const headers = [
        "POST /upload HTTP/1.1",
        "Host: localhost",
        "Content-Type: text/plain",
        "Transfer-Encoding: chunked",
        "",
        "",
      ].join("\r\n");

      client.write(headers);

      const chunk = "A".repeat(100);
      let sentChunks = 0;

      function writeChunk() {
        const chunkHeader = `${chunk.length.toString(16)}\r\n`;
        client.write(chunkHeader);
        client.write(chunk);
        client.write("\r\n");
        sentChunks++;

        if (sentChunks >= 3) {
          client.destroy();
        } else {
          setTimeout(writeChunk, 10);
        }
      }
      writeChunk();
    });
  });

  await deferred1.promise;
  assert(requestError !== null, "Server should have received an error");
  assert(
    (requestError! as Error)?.name === "Http",
    `Expected Http error, got ${(requestError! as Error)?.name}`,
  );
  assert(
    (requestError! as Error)?.message.includes(
      "error reading a body from connection",
    ),
  );
  assertEquals(server.listening, true);
  server.close();
  assertEquals(server.listening, false);
  clearInterval(interval!);
});

Deno.test("[node/http] http.request() post streaming body works", async () => {
  const server = http.createServer((req, res) => {
    if (req.method === "POST") {
      let receivedBytes = 0;
      req.on("data", (chunk) => {
        receivedBytes += chunk.length;
      });
      req.on("end", () => {
        res.writeHead(200, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ bytes: receivedBytes }));
      });
    } else {
      res.writeHead(405, { "Content-Type": "text/plain" });
      res.end("Method Not Allowed");
    }
  });

  const responseEnded = Promise.withResolvers<void>();
  const fileClosed = Promise.withResolvers<void>();
  const timeout = setTimeout(() => {
    responseEnded.reject(new Error("timeout"));
  }, 5000);
  server.listen(0, () => {
    // deno-lint-ignore no-explicit-any
    const port = (server.address() as any).port;
    const filePath = relative(
      Deno.cwd(),
      fromFileUrl(new URL("./testdata/lorem_ipsum_512kb.txt", import.meta.url)),
    );
    const contentLength = 524289;

    const options = {
      hostname: "localhost",
      port: port,
      path: "/",
      method: "POST",
      headers: {
        "Content-Type": "application/octet-stream",
        "Content-Length": contentLength,
      },
    };

    const req = http.request(options, (res) => {
      let responseBody = "";
      res.on("data", (chunk) => {
        responseBody += chunk;
      });

      res.on("end", () => {
        const response = JSON.parse(responseBody);
        assertEquals(res.statusCode, 200);
        assertEquals(response.bytes, contentLength);
        responseEnded.resolve();
      });
    });

    req.on("error", (e) => {
      console.error(`Problem with request: ${e.message}`);
    });

    const readStream = fs.createReadStream(filePath);
    readStream.pipe(req);
    readStream.on("close", fileClosed.resolve);
  });
  await responseEnded.promise;
  await fileClosed.promise;
  assertEquals(server.listening, true);
  server.close();
  clearTimeout(timeout);
  assertEquals(server.listening, false);
});

// https://github.com/denoland/deno/issues/24239
Deno.test("[node/http] ServerResponse write transfer-encoding chunked", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const server = http.createServer((_req, res) => {
    res.setHeader("Content-Type", "text/event-stream");
    res.setHeader("Cache-Control", "no-cache");
    res.setHeader("Connection", "keep-alive");
    res.setHeader("Transfer-Encoding", "chunked");
    res.setHeader("Access-Control-Allow-Origin", "*");

    res.writeHead(200, {
      "Other-Header": "value",
    });
    res.write("");
  });

  server.listen(async () => {
    const { port } = server.address() as { port: number };
    const res = await fetch(`http://localhost:${port}`);
    assertEquals(res.status, 200);
    assertEquals(res.headers.get("content-type"), "text/event-stream");
    assertEquals(res.headers.get("Other-Header"), "value");
    await res.body!.cancel();

    server.close(() => {
      resolve();
    });
  });

  await promise;
});

Deno.test("[node/http] Server.address() can be null", () => {
  const server = http.createServer((_req, res) => res.end("it works"));
  assertEquals(server.address(), null);
});

Deno.test("[node/http] ClientRequest PUT subarray", async () => {
  const buffer = Buffer.from("hello world");
  const payload = buffer.subarray(6, 11);
  let body = "";
  const { promise, resolve, reject } = Promise.withResolvers<void>();
  const req = http.request("http://localhost:4545/echo_server", {
    method: "PUT",
  }, (resp) => {
    resp.on("data", (chunk) => {
      body += chunk;
    });

    resp.on("end", () => {
      resolve();
    });
  });
  req.once("error", (e) => reject(e));
  req.end(payload);
  await promise;
  assertEquals(body, "world");
});

Deno.test("[node/http] req.url equals pathname + search", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();

  const server = http.createServer((req, res) => res.end(req.url));
  server.listen(async () => {
    const { port } = server.address() as net.AddressInfo;
    const res = await fetch(`http://localhost:${port}/foo/bar?baz=1`);
    const text = await res.text();
    assertEquals(text, "/foo/bar?baz=1");

    server.close(() => {
      resolve();
    });
  });

  await promise;
});

Deno.test("[node/http] ClientRequest content-disposition header works", async () => {
  const payload = Buffer.from("hello world");
  let body = "";
  let headers = {} as http.IncomingHttpHeaders;
  const { promise, resolve, reject } = Promise.withResolvers<void>();
  const req = http.request("http://localhost:4545/echo_server", {
    method: "PUT",
    headers: {
      "content-disposition": "attachment",
    },
  }, (resp) => {
    headers = resp.headers;
    resp.on("data", (chunk) => {
      body += chunk;
    });

    resp.on("end", () => {
      resolve();
    });
  });
  req.once("error", (e) => reject(e));
  req.end(payload);
  await promise;
  assertEquals(body, "hello world");
  assertEquals(headers["content-disposition"], "attachment");
});

Deno.test("[node/http] In ClientRequest, option.hostname has precedence over options.host", async () => {
  const responseReceived = Promise.withResolvers<void>();

  new http.ClientRequest({
    hostname: "localhost",
    host: "invalid-hostname.test",
    port: 4545,
    path: "/http_version",
  }).on("response", async (res) => {
    assertEquals(res.statusCode, 200);
    assertEquals(await text(res), "HTTP/1.1");
    responseReceived.resolve();
  }).end();

  await responseReceived.promise;
});

Deno.test("[node/http] upgraded socket closes when the server closed without closing handshake", async () => {
  const clientSocketClosed = Promise.withResolvers<void>();
  const serverProcessClosed = Promise.withResolvers<void>();

  // Uses the server in different process to shutdown it without closing handshake
  const server = `
    Deno.serve({ port: 1337 }, (req) => {
      if (req.headers.get("upgrade") != "websocket") {
        return new Response("ok");
      }
      console.log("upgrade on server");
      const { socket, response } = Deno.upgradeWebSocket(req);
      socket.addEventListener("message", (event) => {
        console.log("server received", event.data);
        socket.send("pong");
      });
      return response;
    });
  `;

  const p = new Deno.Command("deno", { args: ["eval", server] }).spawn();

  // Wait for the server to respond
  await retry(async () => {
    const resp = await fetch("http://localhost:1337");
    const _text = await resp.text();
  });

  const options = {
    port: 1337,
    host: "127.0.0.1",
    headers: {
      "Connection": "Upgrade",
      "Upgrade": "websocket",
      "Sec-WebSocket-Key": "dGhlIHNhbXBsZSBub25jZQ==",
    },
  };

  http.request(options).on("upgrade", (_res, socket) => {
    socket.on("close", () => {
      console.log("client socket closed");
      clientSocketClosed.resolve();
    });
    socket.on("data", async (data) => {
      // receives pong message
      assertEquals(data, Buffer.from("8104706f6e67", "hex"));

      p.kill();
      await p.status;

      console.log("process closed");
      serverProcessClosed.resolve();

      // sending some additional message
      socket.write(Buffer.from("81847de88e01", "hex"));
      socket.write(Buffer.from("0d81e066", "hex"));
    });

    // sending ping message
    socket.write(Buffer.from("81847de88e01", "hex"));
    socket.write(Buffer.from("0d81e066", "hex"));
  }).end();

  await clientSocketClosed.promise;
  await serverProcessClosed.promise;
});

// deno-lint-ignore require-await
Deno.test("[node/http] ServerResponse.call()", async () => {
  function Wrapper(this: unknown, req: IncomingMessage) {
    ServerResponse.call(this, req);
  }
  Object.setPrototypeOf(Wrapper.prototype, ServerResponse.prototype);

  // deno-lint-ignore no-explicit-any
  const wrapper = new (Wrapper as any)(new IncomingMessage(new Socket()));

  assert(wrapper instanceof ServerResponse);
});

Deno.test("[node/http] ServerResponse _header", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const server = http.createServer((_req, res) => {
    assert(Object.hasOwn(res, "_header"));
    res.end();
  });

  server.listen(async () => {
    const { port } = server.address() as { port: number };
    const res = await fetch(`http://localhost:${port}`);
    await res.body?.cancel();
    server.close(() => {
      resolve();
    });
  });

  await promise;
});

Deno.test("[node/http] ServerResponse connection", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const server = http.createServer((_req, res) => {
    assert(Object.hasOwn(res, "connection"));
    assert(res.connection instanceof Socket);
    res.end();
  });

  server.listen(async () => {
    const { port } = server.address() as { port: number };
    const res = await fetch(`http://localhost:${port}`);
    await res.body?.cancel();
    server.close(() => {
      resolve();
    });
  });

  await promise;
});

Deno.test("[node/http] ServerResponse socket", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const server = http.createServer((_req, res) => {
    assert(Object.hasOwn(res, "socket"));
    assert(res.socket instanceof Socket);
    res.end();
  });

  server.listen(async () => {
    const { port } = server.address() as { port: number };
    const res = await fetch(`http://localhost:${port}`);
    await res.body?.cancel();
    server.close(() => {
      resolve();
    });
  });

  await promise;
});

Deno.test("[node/http] decompress brotli response", {
  permissions: { net: true },
}, async () => {
  let received = false;
  const ac = new AbortController();
  const server = Deno.serve({ port: 5928, signal: ac.signal }, (_req) => {
    received = true;
    return Response.json([
      ["accept-language", "*"],
      ["host", "localhost:3000"],
      ["user-agent", "Deno/2.1.1"],
    ], {});
  });
  const { promise, resolve, reject } = Promise.withResolvers<void>();
  let body = "";

  const request = http.get(
    "http://localhost:5928/",
    {
      headers: {
        "accept-encoding": "gzip, deflate, br, zstd",
      },
    },
    (resp) => {
      const decompress = zlib.createBrotliDecompress();
      resp.on("data", (chunk) => {
        decompress.write(chunk);
      });

      resp.on("end", () => {
        decompress.end();
      });

      decompress.on("data", (chunk) => {
        body += chunk;
      });

      decompress.on("end", () => {
        resolve();
      });
    },
  );
  request.on("error", reject);
  request.end(() => {
    assert(received);
  });

  await promise;
  ac.abort();
  await server.finished;

  assertEquals(JSON.parse(body), [["accept-language", "*"], [
    "host",
    "localhost:3000",
  ], ["user-agent", "Deno/2.1.1"]]);
});

Deno.test("[node/http] an error with DNS propagates to request object", async () => {
  const { resolve, promise } = Promise.withResolvers<void>();
  const req = http.request("http://invalid-hostname.test", () => {});
  req.on("error", (err) => {
    assertEquals(err.name, "Error");
    assertEquals(err.message, "getaddrinfo ENOTFOUND invalid-hostname.test");
    resolve();
  });
  await promise;
});

Deno.test("[node/http] supports proxy http request", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const server = Deno.serve({ port: 0, onListen }, (req) => {
    console.log("server received", req.url);
    assertEquals(req.url, "http://example.com/");
    return new Response("ok");
  });

  function onListen({ port }: { port: number }) {
    http.request({
      host: "localhost",
      port,
      path: "http://example.com",
    }, async (res) => {
      assertEquals(res.statusCode, 200);
      assertEquals(await text(res), "ok");
      resolve();
      server.shutdown();
    }).end();
  }
  await promise;
  await server.finished;
});

Deno.test("[node/http] `request` requires net permission to host and port", {
  permissions: { net: ["localhost:4545"] },
}, async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  http.request("http://localhost:4545/echo.ts", async (res) => {
    assertEquals(res.statusCode, 200);
    assertStringIncludes(await text(res), "function echo(");
    resolve();
  }).end();
  await promise;
});

const ca = await Deno.readTextFile("tests/testdata/tls/RootCA.pem");

Deno.test("[node/https] `request` requires net permission to host and port", {
  permissions: { net: ["localhost:5545"] },
}, async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  https.request("https://localhost:5545/echo.ts", { ca }, async (res) => {
    assertEquals(res.statusCode, 200);
    assertStringIncludes(await text(res), "function echo(");
    resolve();
  }).end();
  await promise;
});

Deno.test(
  "[node/http] `request` errors with EPERM error when permission is not granted",
  { permissions: { net: ["localhost:4321"] } }, // wrong permission
  async () => {
    const { promise, resolve } = Promise.withResolvers<void>();
    http.request("http://localhost:4545/echo.ts", async () => {})
      .on("error", (e) => {
        assertEquals(e.message, "getaddrinfo EPERM localhost");
        // deno-lint-ignore no-explicit-any
        assertEquals((e as any).code, "EPERM");
        resolve();
      }).end();
    await promise;
  },
);

Deno.test("[node/http] 'close' event is emitted when request finished", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  let socketCloseEmitted = false;
  const req = http.request("http://localhost:4545/echo.ts", async (res) => {
    res.on("close", resolve);
    req.socket?.on("close", () => {
      socketCloseEmitted = true;
    });
    await text(res);
  });
  req.end();
  await promise;
  assert(socketCloseEmitted);
});

Deno.test("[node/http] 'close' event is emitted on ServerResponse object when the client aborted the request in the middle", async () => {
  let responseCloseEmitted = false;
  const { promise, resolve } = Promise.withResolvers<void>();
  const server = http.createServer((req, res) => {
    res.on("close", () => {
      responseCloseEmitted = true;
      res.end();
    });

    // Streams thre response body
    res.writeHead(200, { "Content-Type": "text/plain" });
    const interval = setInterval(() => {
      res.write("Hello, world!\n");
    }, 100);

    req.on("error", () => {
      clearInterval(interval);
      resolve();
    });
  });

  server.listen(0, () => {
    const { port } = server.address() as { port: number };
    const client = net.createConnection({ port });
    client.write("GET / HTTP/1.1\r\n");
    client.write("Host: localhost\r\n");
    client.write("Connection: close\r\n");
    client.write("\r\n");
    client.on("data", () => {
      // Client aborts the request in the middle
      client.end();
    });
  });

  await promise;
  await new Promise((resolve) => server.close(resolve));
  assert(responseCloseEmitted);
});
