// Copyright 2018-2026 the Deno authors. MIT license.
//
// Tests for Pipe/TCP stream I/O through net.createServer/connect.
// Uses TCP (not unix sockets) so the tests work on all platforms.

import { assertEquals } from "@std/assert";
import { Buffer } from "node:buffer";
import * as net from "node:net";

Deno.test({
  name: "net server and client communicate through pipe",
  sanitizeResources: false,
  sanitizeOps: false,
}, async () => {
  const { promise, resolve } = Promise.withResolvers<string>();

  const server = net.createServer((conn) => {
    let data = "";
    conn.on("data", (chunk: Buffer) => data += chunk.toString());
    conn.on("end", () => {
      resolve(data);
      server.close();
    });
  });

  await new Promise<void>((r) => server.listen(0, "127.0.0.1", r));
  const { port } = server.address() as net.AddressInfo;

  const client = net.connect(port, "127.0.0.1", () => {
    client.write("hello from pipe fd test");
    client.end();
  });

  const result = await promise;
  assertEquals(result, "hello from pipe fd test");
});

Deno.test({
  name: "net bidirectional communication",
  sanitizeResources: false,
  sanitizeOps: false,
}, async () => {
  const { promise, resolve } = Promise.withResolvers<string>();

  const server = net.createServer((conn) => {
    conn.write("pong");
    conn.end();
  });

  await new Promise<void>((r) => server.listen(0, "127.0.0.1", r));
  const { port } = server.address() as net.AddressInfo;

  const client = net.connect(port, "127.0.0.1", () => {
    let data = "";
    client.on("data", (chunk: Buffer) => data += chunk.toString());
    client.on("end", () => {
      resolve(data);
      server.close();
    });
  });

  const result = await promise;
  assertEquals(result, "pong");
});
