// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

import * as http2 from "node:http2";
import { Buffer } from "node:buffer";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import * as net from "node:net";
import { assert, assertEquals } from "@std/assert";
import { curlRequest } from "../unit/test_util.ts";
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);

// Increase the timeout for the auto select family to avoid flakiness
net.setDefaultAutoSelectFamilyAttemptTimeout(
  net.getDefaultAutoSelectFamilyAttemptTimeout() * 30,
);

Deno.test("[node/http2.createServer()]", {
  // TODO(satyarohith): enable the test on windows.
  ignore: Deno.build.os === "windows",
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
  console.log(response);
  assertEquals(response, "Hello, World!");
  server.close();
  // Wait to avoid leaking the timer from here
  // https://github.com/denoland/deno/blob/749b6e45e58ac87188027f79fe403d130f86bd73/ext/node/polyfills/net.ts#L2396-L2402
  // Issue: https://github.com/denoland/deno/issues/22764
  await new Promise<void>((resolve) => server.on("close", resolve));
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
