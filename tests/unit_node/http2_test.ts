// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

import * as http2 from "node:http2";
import * as https from "node:https";
import { AsyncLocalStorage } from "node:async_hooks";
import { Buffer } from "node:buffer";
import fs from "node:fs";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import * as net from "node:net";
import { assert, assertEquals, assertRejects } from "@std/assert";
import { curlRequest } from "../unit/test_util.ts";
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);

// Increase the timeout for the auto select family to avoid flakiness
net.setDefaultAutoSelectFamilyAttemptTimeout(
  net.getDefaultAutoSelectFamilyAttemptTimeout() * 30,
);

for (const url of ["http://localhost:4246", "https://localhost:4247"]) {
  Deno.test(`[node/http2 client] ${url}`, {
    // TODO(littledivy): h2 over TLS is not yet implemented
    ignore: Deno.build.os === "windows" || url.startsWith("https://"),
  }, async () => {
    // Create a server to respond to the HTTP2 requests
    const client = http2.connect(url, {});
    client.on("error", (err) => console.error(err));

    const req = client.request({ ":method": "POST", ":path": "/" }, {
      waitForTrailers: true,
    });

    let receivedTrailers;
    let receivedHeaders;
    let receivedData = "";

    req.on("response", (headers, _flags) => {
      receivedHeaders = headers;
    });

    req.write("hello");
    req.setEncoding("utf8");

    req.on("wantTrailers", () => {
      req.sendTrailers({ foo: "bar" });
    });

    req.on("trailers", (trailers, _flags) => {
      receivedTrailers = trailers;
    });

    req.on("data", (chunk) => {
      receivedData += chunk;
    });

    const { promise, resolve } = Promise.withResolvers<void>();
    req.on("end", () => {
      resolve();
    });
    req.end();

    await promise;
    client.close();
    assertEquals(receivedHeaders?.[":status"], 200);
    assertEquals(receivedData, "hello world\n");

    assertEquals(receivedTrailers?.["abc"], "def");
    assertEquals(receivedTrailers?.["opr"], "stv");
    assertEquals(receivedTrailers?.["foo"], "bar");
    assertEquals(receivedTrailers?.["req_body_len"], "5");
  });
}

Deno.test(`[node/http2 client createConnection]`, async () => {
  const url = "http://127.0.0.1:4246";
  const createConnDeferred = Promise.withResolvers<void>();
  // Create a server to respond to the HTTP2 requests
  const client = http2.connect(url, {
    createConnection() {
      const socket = net.connect({ host: "127.0.0.1", port: 4246 });

      socket.on("connect", () => {
        createConnDeferred.resolve();
      });

      return socket;
    },
  });
  client.on("error", (err) => console.error(err));

  const req = client.request({ ":method": "POST", ":path": "/" });

  let receivedData = "";

  req.write("hello");
  req.setEncoding("utf8");

  req.on("data", (chunk) => {
    receivedData += chunk;
  });

  const endPromise = Promise.withResolvers<void>();
  req.on("end", () => {
    endPromise.resolve();
  });
  req.end();

  await createConnDeferred.promise;
  await endPromise.promise;
  client.close();
  assertEquals(receivedData, "hello world\n");
});

// https://github.com/denoland/deno/issues/29956
Deno.test(`[node/http2 client body overflow]`, async () => {
  const url = "http://127.0.0.1:4246";
  const createConnDeferred = Promise.withResolvers<void>();
  // Create a server to respond to the HTTP2 requests
  const client = http2.connect(url, {
    createConnection() {
      const socket = net.connect({ host: "127.0.0.1", port: 4246 });

      socket.on("connect", () => {
        createConnDeferred.resolve();
      });

      return socket;
    },
  });
  client.on("error", (err) => console.error(err));

  const req = client.request({ ":method": "POST", ":path": "/" });

  let receivedData = "";
  let receivedTrailers;

  const ab = new ArrayBuffer(100);
  const view = new Uint8Array(ab, 0, 5);

  req.write(view);
  req.setEncoding("utf8");

  req.on("data", (chunk) => {
    receivedData += chunk;
  });

  req.on("trailers", (trailers, _flags) => {
    receivedTrailers = trailers;
  });

  const endPromise = Promise.withResolvers<void>();
  req.on("end", () => {
    endPromise.resolve();
  });
  req.end();

  await createConnDeferred.promise;
  await endPromise.promise;
  client.close();
  assertEquals(receivedData, "hello world\n");

  assertEquals(receivedTrailers?.["req_body_len"], "5");
});

Deno.test("[node/http2 client GET https://www.example.com]", {
  // TODO(littledivy): h2 over TLS is not yet implemented
  ignore: true,
}, async () => {
  const clientSession = http2.connect("https://www.example.com");
  const req = clientSession.request({
    ":method": "GET",
    ":path": "/",
  });
  let headers = {};
  let status: number | undefined = 0;
  let chunk = new Uint8Array();
  const endPromise = Promise.withResolvers<void>();
  req.on("response", (h) => {
    status = h[":status"];
    headers = h;
  });
  req.on("data", (c) => {
    chunk = c;
  });
  req.on("end", () => {
    clientSession.close();
    req.close();
    endPromise.resolve();
  });
  req.end();
  await endPromise.promise;
  assert(Object.keys(headers).length > 0);
  assertEquals(status, 200);
  assert(chunk.length > 0);
});

Deno.test("[node/http2.createServer()]", {
  // TODO(satyarohith): enable the test on windows.
  ignore: Deno.build.os === "windows",
  // TODO(littledivy): fix timer leak in http2 server implementation
  sanitizeResources: false,
  sanitizeOps: false,
}, async () => {
  const serverListening = Promise.withResolvers<number>();
  const server = http2.createServer((_req, res) => {
    res.setHeader("Content-Type", "text/html");
    res.setHeader("X-Foo", "bar");
    res.writeHead(200, { "Content-Type": "text/plain; charset=utf-8" });
    res.write("Hello, World!");
    res.end();
  });
  server.listen(0, () => {
    serverListening.resolve((server.address() as net.AddressInfo).port);
  });
  const port = await serverListening.promise;
  const endpoint = `http://localhost:${port}`;

  const response = await curlRequest([
    endpoint,
    "--http2-prior-knowledge",
  ]);
  assertEquals(response, "Hello, World!");
  server.close();
  // Wait to avoid leaking the timer from here
  // https://github.com/denoland/deno/blob/749b6e45e58ac87188027f79fe403d130f86bd73/ext/node/polyfills/net.ts#L2396-L2402
  // Issue: https://github.com/denoland/deno/issues/22764
  await new Promise<void>((resolve) => server.on("close", resolve));
});

Deno.test(
  "[node/http2.createServer()] maxSendHeaderBlockLength keeps header validation enabled",
  {
    ignore: Deno.build.os === "windows",
  },
  async () => {
    const server = http2.createServer({ maxSendHeaderBlockLength: 10000 });
    const portDeferred = Promise.withResolvers<number>();
    const streamDeferred = Promise.withResolvers<void>();

    server.on("stream", (stream, headers) => {
      try {
        assertEquals(headers["x-inject"], undefined);
        stream.respond({ ":status": 204 });
        stream.end();
        streamDeferred.resolve();
      } catch (err) {
        streamDeferred.reject(err);
      }
    });
    server.listen(0, "127.0.0.1", () => {
      const address = server.address() as net.AddressInfo;
      portDeferred.resolve(address.port);
    });

    const port = await portDeferred.promise;
    const conn = await Deno.connect({ hostname: "127.0.0.1", port });
    try {
      await writeHttp2ClientPreface(conn);
      await writeHttp2Headers(
        conn,
        [
          [":method", "GET"],
          [":path", "/"],
          [":authority", `127.0.0.1:${port}`],
          [":scheme", "http"],
          ["x-inject", "injected\r\nset-cookie: session=hacked"],
        ],
        true,
      );
      await streamDeferred.promise;
    } finally {
      conn.close();
      await new Promise<void>((resolve) => server.close(() => resolve()));
    }
  },
);

Deno.test("[node/http2 client] write image buffer on request stream works", {
  // TODO(littledivy): h2 over TLS is not yet implemented
  ignore: true,
}, async () => {
  const url = "https://localhost:5545";
  const client = http2.connect(url);
  client.on("error", (err) => console.error(err));

  const imagePath = join(import.meta.dirname!, "testdata", "green.jpg");
  const buffer = await readFile(imagePath);
  const req = client.request({ ":method": "POST", ":path": "/echo_server" });
  req.write(buffer, (err) => {
    if (err) throw err;
  });

  let receivedData: Buffer;
  req.on("data", (chunk) => {
    if (!receivedData) {
      receivedData = chunk;
    } else {
      receivedData = Buffer.concat([receivedData, chunk]);
    }
  });
  req.end();

  const endPromise = Promise.withResolvers<void>();
  setTimeout(() => {
    try {
      client.close();
    } catch (_) {
      // pass
    }
    endPromise.resolve();
  }, 2000);

  await endPromise.promise;
  assertEquals(receivedData!, buffer);
});

Deno.test("[node/http2 client] write 512kb buffer on request stream works", {
  // TODO(littledivy): h2 over TLS is not yet implemented
  ignore: true,
}, async () => {
  const url = "https://localhost:5545";
  const client = http2.connect(url);
  client.on("error", (err) => console.error(err));

  const filePath = join(
    import.meta.dirname!,
    "testdata",
    "lorem_ipsum_512kb.txt",
  );
  const buffer = await readFile(filePath);
  const req = client.request({ ":method": "POST", ":path": "/echo_server" });
  req.write(buffer, (err) => {
    if (err) throw err;
  });

  let receivedData: Buffer;
  req.on("data", (chunk) => {
    if (!receivedData) {
      receivedData = chunk;
    } else {
      receivedData = Buffer.concat([receivedData, chunk]);
    }
  });
  req.end();

  const endPromise = Promise.withResolvers<void>();
  setTimeout(() => {
    try {
      client.close();
    } catch (_) {
      // pass
    }
    endPromise.resolve();
  }, 2000);

  await endPromise.promise;
  assertEquals(receivedData!, buffer);
});

// https://github.com/denoland/deno/issues/24678
Deno.test("[node/http2 client] deno doesn't panic on uppercase headers", async () => {
  const url = "http://127.0.0.1:4246";
  const client = http2.connect(url);
  client.on("error", (err) => console.error(err));

  // The "User-Agent" header has uppercase characters to test the panic.
  const req = client.request({
    ":method": "POST",
    ":path": "/",
    "User-Agent": "http2",
  });
  const endPromise = Promise.withResolvers<void>();

  let receivedData = "";

  req.write("hello");
  req.setEncoding("utf8");

  req.on("data", (chunk) => {
    receivedData += chunk;
  });
  req.on("end", () => {
    req.close();
    client.close();
    endPromise.resolve();
  });
  req.end();
  await endPromise.promise;
  assertEquals(receivedData, "hello world\n");
});

Deno.test("[node/http2 ClientHttp2Session.socket]", async () => {
  const url = "http://127.0.0.1:4246";
  const client = http2.connect(url);
  client.on("error", (err) => console.error(err));

  const req = client.request({ ":method": "POST", ":path": "/" });
  const endPromise = Promise.withResolvers<void>();

  // test that we can access session.socket
  client.socket.setTimeout(10000);
  // nodejs allows setting arbitrary properties
  // deno-lint-ignore no-explicit-any
  (client.socket as any).nonExistant = 9001;
  // deno-lint-ignore no-explicit-any
  assertEquals((client.socket as any).nonExistant, 9001);

  // regular request dance to make sure it keeps working
  let receivedData = "";
  req.write("hello");
  req.setEncoding("utf8");

  req.on("data", (chunk) => {
    receivedData += chunk;
  });
  req.on("end", () => {
    req.close();
    client.close();
    endPromise.resolve();
  });
  req.end();
  await endPromise.promise;
  assertEquals(client.socket.remoteAddress, "127.0.0.1");
  assertEquals(client.socket.remotePort, 4246);
  assertEquals(client.socket.remoteFamily, "IPv4");
  client.socket.setTimeout(0);
  assertEquals(receivedData, "hello world\n");
});

Deno.test("[node/http2 client] connection states", async () => {
  const expected = {
    beforeConnect: { connecting: true, closed: false, destroyed: false },
    afterConnect: { connecting: false, closed: false, destroyed: false },
    afterClose: { connecting: false, closed: true, destroyed: false },
    afterDestroy: { connecting: false, closed: true, destroyed: true },
  };
  const actual: Partial<typeof expected> = {};

  const url = "http://127.0.0.1:4246";
  const connectPromise = Promise.withResolvers<void>();
  const client = http2.connect(url, {}, () => {
    connectPromise.resolve();
  });
  client.on("error", (err) => console.error(err));

  // close event happens after destory has been called
  const destroyPromise = Promise.withResolvers<void>();
  client.on("close", () => {
    destroyPromise.resolve();
  });

  actual.beforeConnect = {
    connecting: client.connecting,
    closed: client.closed,
    destroyed: client.destroyed,
  };

  await connectPromise.promise;
  actual.afterConnect = {
    connecting: client.connecting,
    closed: client.closed,
    destroyed: client.destroyed,
  };

  // leave a request open to prevent immediate destroy
  const req = client.request();
  req.on("data", () => {});
  req.on("error", (err) => console.error(err));
  const reqClosePromise = Promise.withResolvers<void>();
  req.on("close", () => {
    reqClosePromise.resolve();
  });

  client.close();
  actual.afterClose = {
    connecting: client.connecting,
    closed: client.closed,
    destroyed: client.destroyed,
  };

  await destroyPromise.promise;
  actual.afterDestroy = {
    connecting: client.connecting,
    closed: client.closed,
    destroyed: client.destroyed,
  };

  await reqClosePromise.promise;

  assertEquals(actual, expected);
});

Deno.test("request and response exports", () => {
  assert(http2.Http2ServerRequest);
  assert(http2.Http2ServerResponse);
});

Deno.test("internal/http2/util exports", () => {
  const util = require("internal/http2/util");
  assert(typeof util.kAuthority === "symbol");
  assert(typeof util.kSensitiveHeaders === "symbol");
  assert(typeof util.kSocket === "symbol");
  assert(typeof util.kProtocol === "symbol");
  assert(typeof util.kProxySocket === "symbol");
  assert(typeof util.kRequest === "symbol");
});

Deno.test("internal/http2/util escapes NUL header value bytes", () => {
  const { assertValidPseudoHeader, buildNgHeaderString } = require(
    "internal/http2/util",
  );
  assertEquals(
    buildNgHeaderString(
      { "user-agent": "good\0x-injected\0bad" },
      assertValidPseudoHeader,
      true,
    ),
    ["user-agent\0good\x01x-injected\x01bad\0\0", 1],
  );
  assertEquals(
    buildNgHeaderString(
      { "x-custom": ["good", "bad\u0100x-injected\u0100bad"] },
      assertValidPseudoHeader,
      true,
    ),
    ["x-custom\0good\0\0x-custom\0bad\x01x-injected\x01bad\0\0", 2],
  );
  assertEquals(
    buildNgHeaderString(
      { ":path": "/ok\0x-injected\0bad" },
      assertValidPseudoHeader,
      true,
    ),
    [":path\0/ok\x01x-injected\x01bad\0\0", 1],
  );
});

Deno.test("[node/http2] Server.address() includes family property", async () => {
  // Test IPv4
  {
    const { promise, resolve } = Promise.withResolvers<void>();
    const server = http2.createServer((_req, res) => {
      res.end("ok");
    });

    server.listen(0, "127.0.0.1", () => {
      const addr = server.address() as net.AddressInfo;
      assertEquals(addr.address, "127.0.0.1");
      assertEquals(addr.family, "IPv4");
      assertEquals(typeof addr.port, "number");
      server.close(() => resolve());
    });

    await promise;
  }

  // Test IPv6
  {
    const { promise, resolve } = Promise.withResolvers<void>();
    const server = http2.createServer((_req, res) => {
      res.end("ok");
    });

    server.listen(0, "::1", () => {
      const addr = server.address() as net.AddressInfo;
      assertEquals(addr.address, "::1");
      assertEquals(addr.family, "IPv6");
      assertEquals(typeof addr.port, "number");
      server.close(() => resolve());
    });

    await promise;
  }
});

Deno.test("[node/http2] createSecureServer with allowHTTP1", {
  ignore: Deno.build.os === "windows",
  sanitizeResources: false,
  sanitizeOps: false,
}, async () => {
  const { promise, resolve, reject } = Promise.withResolvers<void>();
  const cert = Deno.readTextFileSync("tests/testdata/tls/localhost.crt");
  const key = Deno.readTextFileSync("tests/testdata/tls/localhost.key");
  const ca = Deno.readTextFileSync("tests/testdata/tls/RootCA.pem");

  // Verifies that createSecureServer with allowHTTP1 doesn't throw
  // ReferenceError for setupConnectionsTracking/httpServerPreClose/HttpServer.
  // TODO(denoland/deno#33317): test HTTP/1.1 fallback once that path works.
  const server = http2.createSecureServer(
    { allowHTTP1: true, cert, key },
    (_req, res) => {
      res.writeHead(200);
      res.end("ok");
    },
  );

  server.listen(0, () => {
    const addr = server.address() as net.AddressInfo;
    const client = http2.connect(`https://localhost:${addr.port}`, { ca });
    client.on("error", reject);
    const req = client.request({ ":path": "/" });
    let data = "";
    req.setEncoding("utf8");
    req.on("data", (chunk: string) => {
      data += chunk;
    });
    req.on("end", () => {
      assertEquals(data, "ok");
      client.close();
      server.close(() => resolve());
    });
    req.on("error", reject);
    req.end();
  });

  await promise;
});

async function writeAll(conn: Deno.Conn, bytes: Uint8Array): Promise<void> {
  let written = 0;
  while (written < bytes.length) {
    written += await conn.write(bytes.subarray(written));
  }
}

function http2LiteralHeader(name: string, value: string): number[] {
  const encoder = new TextEncoder();
  const encodedName = encoder.encode(name);
  const encodedValue = encoder.encode(value);
  return [
    0x00,
    encodedName.length,
    ...encodedName,
    encodedValue.length,
    ...encodedValue,
  ];
}

function http2Frame(
  type: number,
  flags: number,
  streamId: number,
  payload: number[] | Uint8Array,
): Uint8Array {
  const length = payload.length;
  return new Uint8Array([
    (length >> 16) & 0xff,
    (length >> 8) & 0xff,
    length & 0xff,
    type,
    flags,
    (streamId >> 24) & 0x7f,
    (streamId >> 16) & 0xff,
    (streamId >> 8) & 0xff,
    streamId & 0xff,
    ...payload,
  ]);
}

async function writeHttp2ClientPreface(conn: Deno.Conn): Promise<void> {
  const encoder = new TextEncoder();
  await writeAll(conn, encoder.encode("PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"));
  await writeAll(conn, http2Frame(0x04, 0, 0, []));
}

async function writeHttp2Headers(
  conn: Deno.Conn,
  headers: [string, string][],
  endStream: boolean,
): Promise<void> {
  const block = headers.flatMap(([name, value]) =>
    http2LiteralHeader(name, value)
  );
  await writeAll(
    conn,
    http2Frame(0x01, endStream ? 0x05 : 0x04, 1, block),
  );
}

Deno.test("[node/http2] createSecureServer responds to client", {
  ignore: Deno.build.os === "windows",
  sanitizeResources: false,
  sanitizeOps: false,
}, async () => {
  const { promise, resolve, reject } = Promise.withResolvers<void>();
  const cert = Deno.readTextFileSync("tests/testdata/tls/localhost.crt");
  const key = Deno.readTextFileSync("tests/testdata/tls/localhost.key");
  const ca = Deno.readTextFileSync("tests/testdata/tls/RootCA.pem");

  const server = http2.createSecureServer({ cert, key }, (_req, res) => {
    res.writeHead(200);
    res.end("hello-tls");
  });

  server.listen(0, () => {
    const addr = server.address() as net.AddressInfo;
    const client = http2.connect(`https://localhost:${addr.port}`, { ca });
    client.on("error", reject);
    const req = client.request({ ":path": "/" });
    let data = "";
    req.setEncoding("utf8");
    req.on("data", (chunk: string) => {
      data += chunk;
    });
    req.on("end", () => {
      assertEquals(data, "hello-tls");
      client.close();
      server.close(() => resolve());
    });
    req.on("error", reject);
    req.end();
  });

  await promise;
});

Deno.test("[node/http2] stream frameError listener does not throw", {
  ignore: Deno.build.os === "windows",
  sanitizeResources: false,
  sanitizeOps: false,
}, async () => {
  const { promise, resolve, reject } = Promise.withResolvers<void>();

  const server = http2.createServer((_req, res) => {
    res.writeHead(200);
    res.end("ok");
  });

  server.listen(0, () => {
    const addr = server.address() as net.AddressInfo;
    const client = http2.connect(`http://localhost:${addr.port}`);
    client.on("error", reject);
    const req = client.request({ ":path": "/" });
    // Adding a frameError listener exercises kSessionFrameErrorListenerCount
    // and should not throw a ReferenceError
    req.once("frameError", () => {});
    let data = "";
    req.setEncoding("utf8");
    req.on("data", (chunk: string) => {
      data += chunk;
    });
    req.on("end", () => {
      assertEquals(data, "ok");
      client.close();
      server.close(() => resolve());
    });
    req.on("error", reject);
    req.end();
  });

  await promise;
});

Deno.test("[node/http2] AsyncLocalStorage propagates per request", {
  ignore: Deno.build.os === "windows",
  sanitizeResources: false,
  sanitizeOps: false,
}, async () => {
  const storage = new AsyncLocalStorage<{ id: number }>();
  const server = http2.createServer();
  server.on("stream", (stream) => {
    stream.respond({
      [http2.constants.HTTP2_HEADER_CONTENT_TYPE]: "text/plain; charset=utf-8",
      [http2.constants.HTTP2_HEADER_STATUS]: 200,
    });
    stream.end("data");
  });

  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, resolve);
  });

  const port = (server.address() as net.AddressInfo).port;
  const client = storage.run(
    { id: 0 },
    () => http2.connect(`http://localhost:${port}`),
  );

  const done = Promise.withResolvers<void>();
  let completed = 0;

  function closeIfDone() {
    completed++;
    if (completed === 2) {
      client.close();
      server.close((err) => err ? done.reject(err) : done.resolve());
    }
  }

  function requestWith(id: number) {
    storage.run({ id }, () => {
      const req = client.request({
        [http2.constants.HTTP2_HEADER_PATH]: "/",
      });
      req.setEncoding("utf8");
      req.on("response", (headers) => {
        assertEquals(
          Number(headers[http2.constants.HTTP2_HEADER_STATUS]),
          200,
        );
        assertEquals(storage.getStore()?.id, id);
      });
      req.on("data", (chunk: string) => {
        assertEquals(chunk, "data");
        assertEquals(storage.getStore()?.id, id);
      });
      req.on("end", () => {
        assertEquals(storage.getStore()?.id, id);
        closeIfDone();
      });
      req.on("error", done.reject);
      req.end();
    });
  }

  client.on("error", done.reject);
  requestWith(1);
  requestWith(2);

  await done.promise;
});

// Regression test for https://github.com/denoland/deno/issues/35947 — a client
// stream must emit 'response' before any body 'data', even when a 'data'
// listener is attached synchronously right after request() and the body arrives
// in the same frame batch as the response headers.
Deno.test("[node/http2 client] 'response' is emitted before 'data'", {
  ignore: Deno.build.os === "windows",
  sanitizeResources: false,
  sanitizeOps: false,
}, async () => {
  const server = http2.createServer();
  server.on("stream", (stream) => {
    stream.respond({
      [http2.constants.HTTP2_HEADER_CONTENT_TYPE]: "text/plain; charset=utf-8",
      [http2.constants.HTTP2_HEADER_STATUS]: 200,
    });
    stream.write("hello ");
    stream.end("world");
  });

  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, resolve);
  });

  const port = (server.address() as net.AddressInfo).port;
  const client = http2.connect(`http://localhost:${port}`);

  const done = Promise.withResolvers<void>();
  const events: string[] = [];

  const req = client.request({ [http2.constants.HTTP2_HEADER_PATH]: "/" });
  req.on("response", () => events.push("response"));
  req.on("data", () => events.push("data"));
  req.on("end", () => {
    try {
      assertEquals(events[0], "response");
      assert(
        events.indexOf("response") < events.indexOf("data"),
        `expected 'response' before 'data', got: ${events.join(",")}`,
      );
    } catch (e) {
      done.reject(e);
      return;
    }
    client.close();
    server.close((err) => err ? done.reject(err) : done.resolve());
  });
  req.on("error", done.reject);
  client.on("error", done.reject);
  req.end();

  await done.promise;
});

Deno.test("[node/http2 client] connect without net permission", {
  permissions: { net: false },
}, async () => {
  await assertRejects(
    () => {
      return new Promise((_resolve, reject) => {
        const client = http2.connect("http://127.0.0.1:4246");
        client.on("error", reject);
      });
    },
    Deno.errors.NotCapable,
  );
});

// https://github.com/denoland/deno/issues/33009
Deno.test("[node/http2 client] connect with pre-created socket", {
  ignore: Deno.build.os === "windows",
  sanitizeResources: false,
  sanitizeOps: false,
}, async () => {
  const server = http2.createServer();
  server.on("stream", (stream) => {
    stream.respond({ ":status": 200, "content-type": "text/plain" });
    stream.end("ok");
  });

  const port = await new Promise<number>((resolve) => {
    server.listen(0, "127.0.0.1", () => {
      resolve((server.address() as net.AddressInfo).port);
    });
  });

  // Pre-create a connected socket before passing to http2.connect()
  // (pattern used by @grpc/grpc-js)
  const socket = await new Promise<net.Socket>((resolve, reject) => {
    const s = net.connect({ host: "127.0.0.1", port }, () => resolve(s));
    s.once("error", reject);
  });

  const client = http2.connect(`http://127.0.0.1:${port}`, {
    createConnection: () => socket,
  });

  await new Promise<void>((resolve, reject) => {
    const timeout = setTimeout(
      () => reject(new Error("remoteSettings timeout")),
      5000,
    );
    client.once("remoteSettings", () => {
      clearTimeout(timeout);
      resolve();
    });
    client.on("error", (err) => {
      clearTimeout(timeout);
      reject(err);
    });
  });

  const req = client.request({ ":method": "GET", ":path": "/" });
  req.end();

  const body = await new Promise<string>((resolve) => {
    let data = "";
    req.setEncoding("utf8");
    req.on("data", (chunk: string) => (data += chunk));
    req.on("end", () => resolve(data));
  });

  assertEquals(body, "ok");
  client.close();
  server.close();
  await new Promise<void>((resolve) => server.on("close", resolve));
});

// A stream write only completes once nghttp2 has framed its bytes, so a peer
// that stops reading holds the write outstanding instead of letting it buffer
// without bound on the native side.
Deno.test("[node/http2] stream writes apply backpressure", async () => {
  const chunk = Buffer.alloc(256 * 1024);
  const backpressured = Promise.withResolvers<void>();
  const drained = Promise.withResolvers<void>();
  let pending = 0;

  const server = http2.createServer();
  server.on("stream", (stream) => {
    stream.respond({ ":status": 200 });
    // The client leaves the response paused, so its flow-control window closes
    // and nghttp2 stops accepting DATA. Write until the Writable pushes back.
    //
    // With the pre-fix synchronous completion, write() always returns true, so
    // this loop never breaks and the test would hang until the runner's
    // timeout rather than failing the assertions below — that timeout is the
    // primary regression signal here.
    while (true) {
      pending++;
      if (!stream.write(chunk, () => pending--)) {
        break;
      }
    }
    stream.once("drain", () => drained.resolve());
    backpressured.resolve();
  });

  const port = await new Promise<number>((resolve) => {
    server.listen(0, "127.0.0.1", () => {
      resolve((server.address() as net.AddressInfo).port);
    });
  });

  const client = http2.connect(`http://127.0.0.1:${port}`);
  client.on("error", () => {});
  const request = client.request({ ":path": "/" });
  request.on("error", () => {});
  request.on("response", () => request.pause());
  request.end();

  try {
    await backpressured.promise;
    // Nothing can drain the write while the window is shut, so its callback
    // must stay outstanding rather than reporting a bogus instant completion.
    await new Promise((resolve) => setTimeout(resolve, 100));
    assert(pending > 0, "write completed even though the peer never read");

    // Reading reopens the window, which releases the write and the producer.
    request.resume();
    await drained.promise;
    assertEquals(pending, 0);
  } finally {
    client.destroy();
    await new Promise<void>((resolve) => server.close(() => resolve()));
  }
});

Deno.test("[node/http2] destroy cleans internal socket references", async () => {
  const server = http2.createServer((_req, res) => {
    res.end("ok");
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const port = (server.address() as net.AddressInfo).port;

  const client = http2.connect(`http://127.0.0.1:${port}`);
  await new Promise<void>((resolve, reject) => {
    client.once("connect", resolve);
    client.once("error", reject);
  });

  const clientSymbols = Object.getOwnPropertySymbols(client);
  const socketSymbol = clientSymbols.find((symbol) =>
    String(symbol) === "Symbol(socket)"
  );
  const handleSymbol = clientSymbols.find((symbol) =>
    String(symbol) === "Symbol(kHandle)"
  );
  if (!socketSymbol || !handleSymbol) {
    throw new Error("missing expected internal http2 symbols");
  }

  const clientRecord = client as unknown as Record<symbol, unknown>;
  const socket = clientRecord[socketSymbol] as net.Socket;
  const handle = clientRecord[handleSymbol] as Record<symbol, unknown>;
  const ownerSymbol = Object.getOwnPropertySymbols(handle).find((symbol) =>
    String(symbol) === "Symbol(ownerSymbol)"
  );

  client.destroy();
  await new Promise<void>((resolve) => client.once("close", resolve));

  assertEquals(clientRecord[handleSymbol], undefined);
  if (ownerSymbol) {
    assertEquals(handle[ownerSymbol], undefined);
  }
  assertEquals(socket.listenerCount("data"), 0);
  assertEquals(socket.listenerCount("error"), 0);
  assertEquals(socket.listenerCount("close"), 0);

  // A user can retain the internal handle through reflection. Late socket
  // events or callbacks must become harmless after native teardown rather
  // than dereferencing the released nghttp2 session.
  const retainedHandle = handle as unknown as {
    receive(data: Uint8Array): void;
    getOutgoingChunk(): Uint8Array;
    hasPendingData(): boolean;
    settings(callback: () => void): boolean;
    ping(payload: Uint8Array): number;
    destroy(): void;
  };
  retainedHandle.receive(new Uint8Array([0]));
  assertEquals(retainedHandle.getOutgoingChunk().byteLength, 0);
  assertEquals(retainedHandle.hasPendingData(), false);
  assertEquals(retainedHandle.settings(() => {}), false);
  assertEquals(retainedHandle.ping(new Uint8Array(8)), -1);
  retainedHandle.destroy();

  await new Promise<void>((resolve) => server.close(() => resolve()));
});

async function testRespondWithCancellation(ownsFd: boolean) {
  const filePath = await Deno.makeTempFile();
  await Deno.writeFile(filePath, new Uint8Array(256 * 1024));

  const originalRead = fs.read;
  const originalClose = fs.close;
  let streamClosed = false;
  let readsAfterClose = 0;
  let closeCalls = 0;

  Object.defineProperty(fs, "read", {
    configurable: true,
    writable: true,
    value: (...args: unknown[]) => {
      if (streamClosed) readsAfterClose++;
      const callbackIndex = args.length - 1;
      const callback = args[callbackIndex] as (...args: unknown[]) => void;
      args[callbackIndex] = (...callbackArgs: unknown[]) => {
        setTimeout(() => callback(...callbackArgs), 3);
      };
      return Reflect.apply(
        originalRead as unknown as (...args: unknown[]) => unknown,
        fs,
        args,
      );
    },
  });
  Object.defineProperty(fs, "close", {
    configurable: true,
    writable: true,
    value: (...args: unknown[]) => {
      closeCalls++;
      return Reflect.apply(
        originalClose as unknown as (...args: unknown[]) => unknown,
        fs,
        args,
      );
    },
  });

  const fd = ownsFd ? undefined : fs.openSync(filePath, "r");
  const streamClose = Promise.withResolvers<void>();
  const server = http2.createServer();
  server.on("stream", (stream) => {
    stream.on("error", () => {});
    stream.once("close", () => {
      streamClosed = true;
      streamClose.resolve();
    });
    if (ownsFd) {
      stream.respondWithFile(filePath);
    } else {
      stream.respondWithFD(fd!);
    }
  });

  let client: http2.ClientHttp2Session | undefined;
  try {
    const port = await new Promise<number>((resolve) => {
      server.listen(0, "127.0.0.1", () => {
        resolve((server.address() as net.AddressInfo).port);
      });
    });
    client = http2.connect(`http://127.0.0.1:${port}`);
    client.on("error", () => {});
    const request = client.request({ ":path": "/" });
    request.on("error", () => {});
    request.on("response", () => {
      request.destroy();
      client!.destroy();
    });
    request.end();

    const timeout = setTimeout(
      () => streamClose.reject(new Error("stream close timeout")),
      5000,
    );
    await streamClose.promise.finally(() => clearTimeout(timeout));
    await new Promise((resolve) => setTimeout(resolve, 150));

    assertEquals(readsAfterClose, 0);
    assertEquals(closeCalls, ownsFd ? 1 : 0);
  } finally {
    client?.destroy();
    await new Promise<void>((resolve) => server.close(() => resolve()));
    if (fd !== undefined) fs.closeSync(fd);
    Object.defineProperty(fs, "read", {
      configurable: true,
      writable: true,
      value: originalRead,
    });
    Object.defineProperty(fs, "close", {
      configurable: true,
      writable: true,
      value: originalClose,
    });
    await Deno.remove(filePath);
  }
}

Deno.test(
  "[node/http2] respondWithFile stops reading and closes owned fd",
  () => testRespondWithCancellation(true),
);

Deno.test(
  "[node/http2] respondWithFD stops reading without closing caller fd",
  () => testRespondWithCancellation(false),
);

// Regression test for https://github.com/denoland/deno/issues/33317
// `http2.createSecureServer({ allowHTTP1: true })` must handle HTTP/1.1
// clients without throwing `ReferenceError: kIncomingMessage is not defined`.
Deno.test("[node/http2] allowHTTP1 fallback handles HTTP/1.1 clients", async () => {
  const cert = Deno.readTextFileSync("tests/testdata/tls/localhost.crt");
  const key = Deno.readTextFileSync("tests/testdata/tls/localhost.key");
  const ca = Deno.readTextFileSync("tests/testdata/tls/RootCA.pem");

  const { promise, resolve } = Promise.withResolvers<void>();

  const server = http2.createSecureServer(
    { allowHTTP1: true, cert, key },
    (_req, res) => {
      res.writeHead(200, { "content-type": "text/plain" });
      res.end("ok");
    },
  );

  server.listen(0, () => {
    const port = (server.address() as net.AddressInfo).port;
    const req = https.request(
      { hostname: "localhost", port, path: "/", method: "GET", ca },
      (res) => {
        let data = "";
        res.setEncoding("utf8");
        res.on("data", (chunk: string) => (data += chunk));
        res.on("end", () => {
          assertEquals(res.statusCode, 200);
          assertEquals(data, "ok");
          server.close(() => resolve());
        });
      },
    );
    req.on("error", (e) => {
      server.close();
      throw e;
    });
    req.end();
  });

  await promise;
});
